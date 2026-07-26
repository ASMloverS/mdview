@echo off
call "C:\Tools\VS2022\Community\VC\Auxiliary\Build\vcvars64.bat" >NUL
cargo %*
