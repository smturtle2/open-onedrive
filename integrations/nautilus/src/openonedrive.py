#!/usr/bin/env python3

import json
import os
import threading
import time

import gi

gi.require_version("Gio", "2.0")
try:
    gi.require_version("Nautilus", "4.0")
except ValueError:
    gi.require_version("Nautilus", "3.0")

from gi.repository import Gio, GLib, GObject, Nautilus


DBUS_SERVICE = "io.github.smturtle2.OpenOneDrive1"
DBUS_PATH = "/io/github/smturtle2/OpenOneDrive1"
DBUS_INTERFACE = "io.github.smturtle2.OpenOneDrive1"
DBUS_TIMEOUT_MS = 3000
STATUS_TTL_SECONDS = 2.0
LISTING_TTL_SECONDS = 2.0
PATH_STATE_TTL_SECONDS = 2.0

STATE_TO_EMBLEM = {
    "PinnedLocal": "open-onedrive-pinned",
    "AvailableLocal": "open-onedrive-local",
    "Syncing": "open-onedrive-syncing",
    "Conflict": "open-onedrive-attention",
    "Error": "open-onedrive-attention",
    "OnlineOnly": "open-onedrive-online-only",
}

STATE_TO_LABEL = {
    "PinnedLocal": "Always available on this device",
    "AvailableLocal": "Available on this device",
    "Syncing": "Syncing",
    "Conflict": "Needs attention",
    "Error": "Error",
    "OnlineOnly": "Online-only",
}


def _normalized_path(path):
    if not path:
        return ""
    return os.path.normpath(path)


def _local_path_for_file(file_info):
    if file_info is None:
        return ""
    try:
        location = file_info.get_location()
    except AttributeError:
        return ""
    if location is None:
        return ""
    path = location.get_path()
    return _normalized_path(path)


def _path_from_relative(root_path, relative_path):
    root_path = _normalized_path(root_path)
    relative_path = str(relative_path or "").strip("/")
    if not root_path:
        return ""
    if not relative_path:
        return root_path
    return _normalized_path(os.path.join(root_path, *relative_path.split("/")))


class _DaemonClient:
    def __init__(self):
        self._lock = threading.Lock()
        self._proxy = None
        self._proxy_signal_connected = False
        self._signal_handlers = []

    def add_signal_handler(self, handler):
        with self._lock:
            self._signal_handlers.append(handler)
            proxy = self._proxy
        if proxy is not None:
            self._ensure_signal_connection(proxy)

    def get_status(self):
        payload = self._call_json("GetStatusJson")
        if isinstance(payload, dict):
            return payload
        return {}

    def get_path_states(self, paths):
        if not paths:
            return []
        payload = self._call_json("GetPathStatesJson", GLib.Variant("(as)", (paths,)))
        if isinstance(payload, list):
            return payload
        return []

    def list_directory(self, raw_path):
        payload = self._call_json("ListDirectoryJson", GLib.Variant("(s)", (raw_path,)))
        if isinstance(payload, list):
            return payload
        return []

    def invoke_action(self, method, paths):
        if not paths:
            return False
        proxy = self._get_proxy()
        if proxy is None:
            return False
        try:
            result = proxy.call_sync(
                method,
                GLib.Variant("(as)", (paths,)),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                None,
            )
        except GLib.Error:
            self._reset_proxy()
            return False

        values = result.unpack()
        if isinstance(values, tuple):
            return bool(values)
        return True

    def _call_json(self, method, parameters=None):
        proxy = self._get_proxy()
        if proxy is None:
            return None
        try:
            result = proxy.call_sync(
                method,
                parameters,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                None,
            )
        except GLib.Error:
            self._reset_proxy()
            return None

        values = result.unpack()
        if isinstance(values, tuple):
            if not values:
                return None
            payload = values[0]
        else:
            payload = values
        if not isinstance(payload, str):
            return None
        try:
            return json.loads(payload)
        except json.JSONDecodeError:
            return None

    def _get_proxy(self):
        with self._lock:
            proxy = self._proxy
        if proxy is not None:
            return proxy

        try:
            proxy = Gio.DBusProxy.new_for_bus_sync(
                Gio.BusType.SESSION,
                Gio.DBusProxyFlags.NONE,
                None,
                DBUS_SERVICE,
                DBUS_PATH,
                DBUS_INTERFACE,
                None,
            )
        except GLib.Error:
            return None

        with self._lock:
            self._proxy = proxy
        self._ensure_signal_connection(proxy)
        return proxy

    def _ensure_signal_connection(self, proxy):
        with self._lock:
            if self._proxy_signal_connected:
                return
            self._proxy_signal_connected = True
        if proxy is None:
            return
        proxy.connect("g-signal", self._on_signal)

    def _on_signal(self, _proxy, _sender_name, signal_name, parameters):
        with self._lock:
            handlers = list(self._signal_handlers)
        for handler in handlers:
            try:
                handler(signal_name, parameters)
            except Exception:
                continue

    def _reset_proxy(self):
        with self._lock:
            self._proxy = None
            self._proxy_signal_connected = False


class _StateCache:
    def __init__(self, client):
        self._client = client
        self._lock = threading.RLock()
        self._status = {}
        self._status_at = 0.0
        self._directory_entries = {}
        self._path_states = {}
        self._tracked_files = {}
        self._client.add_signal_handler(self._on_signal)

    def get_status(self):
        now = time.monotonic()
        with self._lock:
            if self._status and now - self._status_at <= STATUS_TTL_SECONDS:
                return dict(self._status)

        status = self._normalize_status(self._client.get_status())
        with self._lock:
            self._status = status
            self._status_at = now
        return dict(status)

    def is_visible_managed_path(self, path):
        path = _normalized_path(path)
        status = self.get_status()
        root_path = status.get("root_path", "")
        backing_dir_name = status.get("backing_dir_name", "")
        if not status.get("remote_configured") or not path or not root_path:
            return False
        if path != root_path and not path.startswith(root_path + os.sep):
            return False
        if backing_dir_name:
            hidden_root = _normalized_path(os.path.join(root_path, backing_dir_name))
            if path == hidden_root or path.startswith(hidden_root + os.sep):
                return False
        return True

    def track_file(self, file_info):
        path = _local_path_for_file(file_info)
        if path:
            with self._lock:
                self._tracked_files[path] = (time.monotonic(), file_info)
                self._prune_tracked_files()

    def path_states_for_selection(self, paths):
        normalized_paths = [_normalized_path(path) for path in paths]
        visible_paths = [path for path in normalized_paths if self.is_visible_managed_path(path)]
        if not visible_paths:
            return {}

        now = time.monotonic()
        missing = []
        states = {}
        with self._lock:
            for path in visible_paths:
                cached = self._path_states.get(path)
                if cached is not None and now - cached[0] <= PATH_STATE_TTL_SECONDS:
                    states[path] = dict(cached[1])
                else:
                    missing.append(path)

        if missing:
            fetched = self._client.get_path_states(missing)
            root_path = self.get_status().get("root_path", "")
            fetched_map = self._map_states(root_path, fetched)
            with self._lock:
                for path, state in fetched_map.items():
                    self._path_states[path] = (now, dict(state))
                    states[path] = dict(state)

        return states

    def state_for_file(self, file_info):
        path = _local_path_for_file(file_info)
        if not self.is_visible_managed_path(path):
            return None

        self.track_file(file_info)
        status = self.get_status()
        root_path = status.get("root_path", "")
        parent_location = file_info.get_parent_location()
        parent_path = _normalized_path(parent_location.get_path() if parent_location else "")

        if parent_path and (parent_path == root_path or self.is_visible_managed_path(parent_path)):
            listing = self._directory_listing(parent_path)
            state = listing.get(path)
            if state is not None:
                return dict(state)

        selection_states = self.path_states_for_selection([path])
        return dict(selection_states.get(path, {})) or None

    def invalidate_paths(self, changed_paths):
        with self._lock:
            if not changed_paths:
                tracked_files = [entry[1] for entry in self._tracked_files.values()]
                self._directory_entries.clear()
                self._path_states.clear()
            else:
                normalized_paths = {_normalized_path(path) for path in changed_paths if path}
                tracked_files = []
                for path in normalized_paths:
                    self._path_states.pop(path, None)
                    self._directory_entries.pop(path, None)
                    self._directory_entries.pop(_normalized_path(os.path.dirname(path)), None)
                    tracked = self._tracked_files.get(path)
                    if tracked is not None:
                        tracked_files.append(tracked[1])

        for file_info in tracked_files:
            try:
                file_info.invalidate_extension_info()
            except Exception:
                continue

    def _directory_listing(self, directory_path):
        directory_path = _normalized_path(directory_path)
        now = time.monotonic()
        with self._lock:
            cached = self._directory_entries.get(directory_path)
            if cached is not None and now - cached[0] <= LISTING_TTL_SECONDS:
                return dict(cached[1])

        status = self.get_status()
        root_path = status.get("root_path", "")
        query_path = "/" if directory_path == root_path else directory_path
        entries = self._map_states(root_path, self._client.list_directory(query_path))
        with self._lock:
            self._directory_entries[directory_path] = (now, dict(entries))
            for path, state in entries.items():
                self._path_states[path] = (now, dict(state))
        return dict(entries)

    def _map_states(self, root_path, states):
        mapped = {}
        if not root_path:
            return mapped
        for state in states:
            if not isinstance(state, dict):
                continue
            absolute_path = _path_from_relative(root_path, state.get("path", ""))
            if absolute_path:
                mapped[absolute_path] = dict(state)
        return mapped

    def _normalize_status(self, status):
        if not isinstance(status, dict):
            return {}
        return {
            "remote_configured": bool(status.get("remote_configured")),
            "root_path": _normalized_path(status.get("root_path")),
            "backing_dir_name": str(status.get("backing_dir_name") or ""),
        }

    def _on_signal(self, signal_name, parameters):
        if signal_name != "PathStatesChanged":
            return

        status = self.get_status()
        root_path = status.get("root_path", "")
        if not root_path:
            self.invalidate_paths([])
            return

        try:
            values = parameters.unpack()
        except Exception:
            self.invalidate_paths([])
            return

        relative_paths = values[0] if isinstance(values, tuple) and values else []
        if not relative_paths:
            self.invalidate_paths([])
            return

        changed_paths = [_path_from_relative(root_path, relative_path) for relative_path in relative_paths]
        self.invalidate_paths(changed_paths)

    def _prune_tracked_files(self):
        if len(self._tracked_files) <= 256:
            return
        oldest_paths = sorted(self._tracked_files.items(), key=lambda item: item[1][0])[:-256]
        for path, _entry in oldest_paths:
            self._tracked_files.pop(path, None)


class OpenOneDriveNautilusExtension(GObject.GObject, Nautilus.MenuProvider, Nautilus.InfoProvider):
    def __init__(self):
        super().__init__()
        self._client = _DaemonClient()
        self._cache = _StateCache(self._client)
        self._client.add_signal_handler(self._handle_menu_signal)

    def get_file_items(self, *args):
        files = list(args[-1]) if args else []
        selected_paths = self._selected_paths(files)
        if not selected_paths:
            return []

        states = self._cache.path_states_for_selection(selected_paths)
        submenu = Nautilus.Menu()
        added = False

        state_values = list(states.values())
        all_online_only = bool(state_values) and all(
            state.get("state") == "OnlineOnly" for state in state_values
        )
        all_local = bool(state_values) and all(
            state.get("state") in ("PinnedLocal", "AvailableLocal")
            for state in state_values
        )
        all_retryable = bool(state_values) and all(
            state.get("state") in ("Conflict", "Error") for state in state_values
        )

        if all_online_only:
            submenu.append_item(
                self._menu_item(
                    "OpenOneDriveKeepLocal",
                    "Keep on this device",
                    "Download and pin the selected items",
                    lambda *_args: self._client.invoke_action("KeepLocal", selected_paths),
                )
            )
            added = True

        if all_local:
            submenu.append_item(
                self._menu_item(
                    "OpenOneDriveMakeOnlineOnly",
                    "Free up space",
                    "Free local space and keep the selected items online",
                    lambda *_args: self._client.invoke_action("MakeOnlineOnly", selected_paths),
                )
            )
            added = True

        if all_retryable:
            submenu.append_item(
                self._menu_item(
                    "OpenOneDriveRetryTransfer",
                    "Retry transfer",
                    "Retry failed or conflicted transfers",
                    lambda *_args: self._client.invoke_action("RetryTransfer", selected_paths),
                )
            )
            added = True

        status = self._cache.get_status()
        root_path = status.get("root_path", "")
        if root_path:
            submenu.append_item(
                self._menu_item(
                    "OpenOneDriveOpenRoot",
                    "Open OneDrive root",
                    "Open the mounted OneDrive root folder",
                    lambda *_args: self._open_path(root_path),
                )
            )
            added = True

        if not added:
            return []

        root_item = Nautilus.MenuItem(
            name="OpenOneDriveRoot",
            label="Open OneDrive",
            tip="Open OneDrive actions",
            icon="folder-cloud",
        )
        root_item.set_submenu(submenu)
        return [root_item]

    def get_background_items(self, *args):
        current_folder = args[-1] if args else None
        current_path = _local_path_for_file(current_folder)
        if not current_path or not self._cache.is_visible_managed_path(current_path):
            return []

        status = self._cache.get_status()
        root_path = status.get("root_path", "")
        if not root_path:
            return []

        item = self._menu_item(
            "OpenOneDriveBackgroundRoot",
            "Open OneDrive root",
            "Open the mounted OneDrive root folder",
            lambda *_args: self._open_path(root_path),
        )
        return [item]

    def update_file_info(self, file_info):
        state = self._cache.state_for_file(file_info)
        if not state:
            return

        state_name = state.get("state", "")
        emblem = STATE_TO_EMBLEM.get(state_name)
        if emblem:
            try:
                file_info.add_emblem(emblem)
            except Exception:
                pass

        label = STATE_TO_LABEL.get(state_name, state_name)
        if label:
            try:
                file_info.add_string_attribute("openonedrive::sync-state", label)
            except Exception:
                pass

        error_message = state.get("error") or state.get("conflict_reason") or ""
        if error_message:
            try:
                file_info.add_string_attribute("openonedrive::detail", str(error_message))
            except Exception:
                pass

    def _selected_paths(self, files):
        paths = []
        for file_info in files:
            self._cache.track_file(file_info)
            path = _local_path_for_file(file_info)
            if not path or not self._cache.is_visible_managed_path(path):
                return []
            paths.append(path)
        return paths

    def _menu_item(self, name, label, tip, callback):
        item = Nautilus.MenuItem(
            name=name,
            label=label,
            tip=tip,
            icon="folder-cloud",
        )
        item.connect("activate", callback)
        return item

    def _handle_menu_signal(self, signal_name, _parameters):
        if signal_name != "PathStatesChanged":
            return
        emit_menu_signal = getattr(Nautilus, "menu_provider_emit_items_updated_signal", None)
        if emit_menu_signal is None:
            return
        try:
            emit_menu_signal(self)
        except Exception:
            pass

    def _open_path(self, path):
        if not path:
            return
        try:
            Gio.AppInfo.launch_default_for_uri(Gio.File.new_for_path(path).get_uri(), None)
        except Exception:
            pass
