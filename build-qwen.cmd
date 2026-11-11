@echo off
rem Compile le moteur qwentts.cpp en bibliotheques partagees (qwen.dll + ggml*.dll).
rem
rem   build-qwen.cmd         Vulkan : AMD, NVIDIA et Intel (version testee sur une RX 6700 XT)
rem   build-qwen.cmd cuda    Vulkan + CUDA : plus rapide sur NVIDIA. NON TESTE (aucune carte NVIDIA ni CUDA sur la machine
rem                          de developpement). Necessite le CUDA Toolkit 12.x (variable CUDA_PATH). VoiceClone detecte
rem                          ggml-cuda.dll au demarrage et retombe sur Vulkan si CUDA ne se charge pas.
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
cd /d %~dp0vendor\qwentts

set CUDA_FLAGS=
if /i "%1"=="cuda" (
  if "%CUDA_PATH%"=="" (
    echo CUDA_PATH est vide : installer le CUDA Toolkit 12.x puis relancer.
    exit /b 1
  )
  rem GGML_NATIVE=OFF : liste d'architectures portable (GTX 10 a RTX 50) au lieu de celle de la carte presente.
  set CUDA_FLAGS=-DGGML_CUDA=ON -DGGML_NATIVE=OFF
  rem Dossier separe : on ne melange pas les DLL des deux variantes.
  set BUILD_DIR=build-cuda
) else (
  set BUILD_DIR=build
)

if not exist %BUILD_DIR% mkdir %BUILD_DIR%
cd %BUILD_DIR%
cmake .. -G Ninja -DCMAKE_BUILD_TYPE=Release -DGGML_VULKAN=ON -DQWEN_SHARED=ON %CUDA_FLAGS% -DCMAKE_C_COMPILER=cl -DCMAKE_CXX_COMPILER=cl || exit /b 1
cmake --build . -j 8 || exit /b 1

if /i "%1"=="cuda" (
  rem DLL d'execution CUDA livrees avec l'application : l'utilisateur n'a pas besoin d'installer le toolkit.
  for %%f in ("%CUDA_PATH%\bin\cudart64_*.dll" "%CUDA_PATH%\bin\cublas64_*.dll" "%CUDA_PATH%\bin\cublasLt64_*.dll") do copy /y "%%~f" . >nul
  echo.
  echo Variante CUDA compilee dans vendor\qwentts\build-cuda. Pour l'utiliser : lancer l'application avec
  echo VOICECLONE_QWEN_BUILD=build-cuda ^(voir README.md^).
)
