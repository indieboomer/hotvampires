@echo off
cd /d "%~dp0"
cargo run --offline --target-dir target/iteration -- %*
