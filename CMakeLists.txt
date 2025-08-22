cmake_minimum_required(VERSION 3.29)
project(Shotmd)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

find_package(Qt6 COMPONENTS
        Core
        Gui
        Widgets
        REQUIRED)

add_executable(Shotmd
        WIN32
        main.cpp
        shotmd.cpp
        shotmd.h
        shotmd.ui
)
target_link_libraries(Shotmd
        Qt::Core
        Qt::Gui
        Qt::Widgets
)

