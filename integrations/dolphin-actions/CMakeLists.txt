add_library(open_onedrive_fileitemaction MODULE
    src/ActionPlugin.cpp
    src/ActionPlugin.hpp
)

target_link_libraries(open_onedrive_fileitemaction
    PRIVATE
        Qt6::Core
        Qt6::DBus
        Qt6::Gui
        Qt6::Widgets
        KF6::CoreAddons
        KF6::KIOWidgets
)

target_include_directories(open_onedrive_fileitemaction PRIVATE src)

set_target_properties(open_onedrive_fileitemaction PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/plugins/kf6/kfileitemaction"
)

install(TARGETS open_onedrive_fileitemaction
    LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}/qt6/plugins/kf6/kfileitemaction"
)
