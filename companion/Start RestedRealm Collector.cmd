@echo off
cd /d "%~dp0"
pythonw.exe gui.py
if errorlevel 1 python.exe gui.py
