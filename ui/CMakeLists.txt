cmake_minimum_required(VERSION 3.28)

project(open-onedrive-ui LANGUAGES CXX)

include(GNUInstallDirs)

set(OpenGL_GL_PREFERENCE LEGACY)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(QT_DEFAULT_MAJOR_VERSION 6)
set(QT_MAJOR_VERSION 6)

find_package(OpenGL REQUIRED)
if(TARGET OpenGL::OpenGL AND NOT TARGET OpenGL::GL)
    add_library(OpenGL::GL INTERFACE IMPORTED)
    target_link_libraries(OpenGL::GL INTERFACE OpenGL::OpenGL)
endif()
find_package(Qt6 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 DBus QuickDialogs2)
find_package(KF6Kirigami REQUIRED)

qt_standard_project_setup(REQUIRES 6.8)

qt_add_executable(open-onedrive-ui
    resources/ui.qrc
    src/main.cpp
    src/ShellBackend.cpp
    src/ShellBackend.h
)

target_link_libraries(open-onedrive-ui
    PRIVATE
        Qt6::Core
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        Qt6::QuickControls2
        Qt6::DBus
        Qt6::QuickDialogs2
        KF6::Kirigami
)

install(TARGETS open-onedrive-ui RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
