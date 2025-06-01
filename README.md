# Shotmd
  shotmd is a small and handy tool to screenshot and convert to base64 code in order to insert image to markdown.
# usage
1. start it. (i recommend you to start it by win key and type "Shotmd"!
2. press and drag to select the part you want, you can exit by right click at any time.
3. after you release the mouse ,the image  will be saved on your disk, and the base64 code will be on your clipboard
# Build 
  I am programming on wsl2 Archlinux, using clion and Qt.
  1. To build for linux: Install Qt by its online installer,and Clion will recognize it, which works for me, or you can try the linux in makefile.(the toolchain arg may be the problem)
  2. To build for MinGW: Install mingw related packages, which Archlinux works well. Install Qt on windows to get windeployqt.exe. run the mingw in makefile.
  3. To build for msvc: Install visual studio 2022.(which works for me,for now,and for this Project) run the msvc in makefile.(warning: due to the wsl2 env ,i need to copy files to win disks to build. Details in makefile)

or natively on windows with toolchain file:
```shell
cmake -S . -B build -G Ninja -DCMAKE_TOOLCHAIN_FILE=C:\Qt\6.9.0\msvc2022_64\lib\cmake\Qt6\qt.toolchain.cmake 
cmake --build build --config Release
C:\Qt\6.9.0\msvc2022_64\bin\windeployqt.exe build\Shotmd.exe
```