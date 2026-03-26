cmake_minimum_required(VERSION 3.28)

project(open-onedrive-integrations LANGUAGES CXX)

include(GNUInstallDirs)

add_subdirectory(nautilus)

set(OpenGL_GL_PREFERENCE LEGACY)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(QT_DEFAULT_MAJOR_VERSION 6)
set(QT_MAJOR_VERSION 6)

find_package(Qt6 QUIET COMPONENTS Core Widgets Gui DBus)
find_package(KF6CoreAddons QUIET)
find_package(KF6KIO QUIET)

if(TARGET Qt6::Core
   AND TARGET Qt6::Widgets
   AND TARGET Qt6::Gui
   AND TARGET Qt6::DBus
   AND TARGET KF6::CoreAddons
   AND TARGET KF6::KIOCore
   AND TARGET KF6::KIOWidgets)
    add_subdirectory(dolphin-actions)
    add_subdirectory(dolphin-overlay)
else()
    message(STATUS "Skipping Dolphin integrations because KDE dependencies are unavailable.")
endif()
