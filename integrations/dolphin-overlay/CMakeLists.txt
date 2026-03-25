add_library(open_onedrive_overlayicon MODULE
    src/OverlayIconPlugin.cpp
    src/OverlayIconPlugin.hpp
)

target_link_libraries(open_onedrive_overlayicon
    PRIVATE
        Qt6::Core
        Qt6::DBus
        KF6::CoreAddons
        KF6::KIOCore
)

target_include_directories(open_onedrive_overlayicon PRIVATE src)

set_target_properties(open_onedrive_overlayicon PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/plugins/kf6/overlayicon"
)

install(TARGETS open_onedrive_overlayicon
    LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}/qt6/plugins/kf6/overlayicon"
)
