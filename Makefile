.PHONY:mingw linux msvc
projectName=$(shell basename $(CURDIR))
projectPath=D:\\linux$(subst /,\\,$(shell pwd))
mingw:
	x86_64-w64-mingw32-cmake -DCMAKE_BUILD_TYPE=Debug -DCMAKE_MAKE_PROGRAM=x86_64-w64-mingw32-make -DCMAKE_C_COMPILER=x86_64-w64-mingw32-gcc -DCMAKE_CXX_COMPILER=x86_64-w64-mingw32-g++ -DCMAKE_TOOLCHAIN_FILE=/mnt/c/Qt/6.7.2/mingw_64/lib/cmake/Qt6/qt.toolchain.cmake -DCMAKE_SHARED_LINKER_FLAGS=-Wl,-undefined -DCMAKE_FIND_USE_CMAKE_SYSTEM_PATH=FALSE -DMAKE_FIND_USE_SYSTEM_ENVIRONMENT_PATH=FALSE -DCMAKE_SYSROOT=/usr/x86_64-w64-mingw32/ -G "Unix Makefiles" -S . -B ./cmake-build-mingw
	cmake --build ./cmake-build-mingw --target all
	cp /usr/x86_64-w64-mingw32/bin/libgcc_s_seh-1.dll ./cmake-build-mingw/
	cp /usr/x86_64-w64-mingw32/bin/libstdc++-6.dll   ./cmake-build-mingw/
	cp /usr/x86_64-w64-mingw32/bin/libwinpthread-1.dll   ./cmake-build-mingw/
	/mnt/c/Qt/6.7.2/mingw_64/bin/windeployqt.exe ./cmake-build-mingw/$(projectName).exe
linux:
	cmake -DCMAKE_BUILD_TYPE=Release -DCMAKE_BUILD_PROGRAME=make -DCMAKE_TOOLCHAIN_FILE=/opt/Qt/6.7.2/gcc_64/lib/cmake/Qt6/qt.toolchain.cmake -G "Unix Makefiles" -S . -B ./cmake-build-release
	cmake --build cmake-build-release --target all
testOnLinux: linux
	cd cmake-build-release/ && ./$(projectName)
testOnWindows: mingw
	cd $(shell pwd)/cmake-build-mingw/ && ./$(projectName).exe
msvc:
	mkdir -p /mnt/d/linux$(shell pwd)
	powershell.exe -c  "& 'C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\Launch-VsDevShell.ps1' -SkipAutomaticLocation;cmake -DCMAKE_BUILD_TYPE=Release  -DCMAKE_TOOLCHAIN_FILE=C:\Qt\6.7.2\msvc2019_64\lib\cmake\Qt6\qt.toolchain.cmake  -DCMAKE_CXX_FLAGS='-std:c++20 -Zc:__cplusplus' -S . -B $(projectPath)\\cmake-build-msvc"
	powershell.exe -c  "& 'C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\Launch-VsDevShell.ps1' -SkipAutomaticLocation;msbuild $(projectPath)\\cmake-build-msvc\\$(projectName).sln /p:Configuration=Release"
	/mnt/c/Qt/6.7.2/msvc2019_64/bin/windeployqt.exe $(projectPath)\\cmake-build-msvc\\Release\\$(projectName).exe
