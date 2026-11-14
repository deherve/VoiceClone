@echo off
rem Compile le serveur Zonos 3-Mini (Zyphra, licence MIT) en version Vulkan et le range dans release-assets\zonos, d'ou
rem scripts\release.ps1 le prend. Aucun binaire Windows n'est publie par Zyphra. Necessite git, CMake, Ninja, Visual Studio
rem Build Tools et une chaine Rust (la normalisation du texte est un module Rust compile par CMake).
rem Version compilee et testee : commit 96d9e0e de https://github.com/Zyphra/ZONOS3-Mini
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
cd /d %~dp0
if not exist vendor\zonos3 git clone --recurse-submodules https://github.com/Zyphra/ZONOS3-Mini.git vendor\zonos3 || exit /b 1
git -C vendor\zonos3 checkout 96d9e0e || exit /b 1
git -C vendor\zonos3 submodule update --init --recursive || exit /b 1
cd vendor\zonos3\servers\ggml
if not exist build-vk mkdir build-vk
cd build-vk
cmake .. -G Ninja -DCMAKE_BUILD_TYPE=Release -DGGML_VULKAN=ON -DCMAKE_C_COMPILER=cl -DCMAKE_CXX_COMPILER=cl || exit /b 1
cmake --build . -j 8 || exit /b 1
set OUT=%~dp0release-assets\zonos
if not exist %OUT% mkdir %OUT%
copy /y zonos3-server.exe %OUT% >nul
for %%f in (ggml.dll ggml-base.dll ggml-cpu.dll ggml-vulkan.dll) do copy /y bin\%%f %OUT% >nul
copy /y ..\..\..\LICENSE %OUT%\zonos3-LICENSE.txt >nul
copy /y ..\..\..\NOTICE %OUT%\zonos3-NOTICE.txt >nul
echo Serveur Zonos 3 pret dans %OUT%
