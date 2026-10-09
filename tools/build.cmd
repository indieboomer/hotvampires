@echo off
cd /d "%~dp0.."
cargo build --offline --release
