// 防止 Windows release 模式下弹出控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    codex_proxy_lib::run()
}
