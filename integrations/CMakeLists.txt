cmake_minimum_required(VERSION 3.28)

project(open-onedrive-integrations LANGUAGES CXX)

include(GNUInstallDirs)

set(OpenGL_GL_PREFERENCE LEGACY)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(QT_DEFAULT_MAJOR_VERSION 6)
set(QT_MAJOR_VERSION 6)

find_package(OpenGL REQUIRED)
if(TARGET OpenGL::OpenGL AND NOT TARGET OpenGL::GL)
    add_library(OpenGL::GL INTERFACE IMPORTED)
    target_link_libraries(OpenGL::GL INTERFACE OpenGL::OpenGL)
endif()
find_package(Qt6 REQUIRED COMPONENTS Core Widgets Gui DBus)
find_package(KF6CoreAddons REQUIRED)
find_package(KF6KIO REQUIRED)

add_subdirectory(dolphin-actions)
