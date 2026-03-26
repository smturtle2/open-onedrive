find_package(Python3 REQUIRED COMPONENTS Interpreter)

set(OPEN_ONEDRIVE_NAUTILUS_EXTENSION
    "${CMAKE_CURRENT_SOURCE_DIR}/src/openonedrive.py"
)

add_custom_target(open_onedrive_nautilus_check ALL
    COMMAND
        "${Python3_EXECUTABLE}" "-c"
        "import os, py_compile, tempfile; handle = tempfile.NamedTemporaryFile(suffix='.pyc', delete=False); handle.close(); py_compile.compile(r'${OPEN_ONEDRIVE_NAUTILUS_EXTENSION}', cfile=handle.name, doraise=True); os.unlink(handle.name)"
    DEPENDS "${OPEN_ONEDRIVE_NAUTILUS_EXTENSION}"
    VERBATIM
)

install(FILES "${OPEN_ONEDRIVE_NAUTILUS_EXTENSION}"
    DESTINATION "${CMAKE_INSTALL_DATADIR}/nautilus-python/extensions"
)
