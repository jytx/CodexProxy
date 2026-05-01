/// Codex config.toml 自动同步模块
/// 启动代理时自动更新 ~/.codex/config.toml 中的 model、model_provider、profiles 等配置
use std::path::PathBuf;
use toml_edit::{Item, Table, value};

/// 将 profile 名称转为有效的 TOML section key（小写+连字符）
fn sanitize_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// 获取 Codex config 路径: ~/.codex/config.toml
fn codex_config_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| "无法确定用户 home 目录".to_string())?;
    Ok(home.join(".codex").join("config.toml"))
}

/// 获取 proxy-env.sh 路径: ~/.codex/proxy-env.sh
fn proxy_env_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| "无法确定用户 home 目录".to_string())?;
    Ok(home.join(".codex").join("proxy-env.sh"))
}

/// 获取 .zshrc 路径
fn zshrc_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| "无法确定用户 home 目录".to_string())?;
    Ok(home.join(".zshrc"))
}

/// 写入环境变量到 proxy-env.sh 并确保 .zshrc 会 source 它
fn set_proxy_env_var(env_key_name: &str) -> Result<(), String> {
    let env_var_key = format!("{}_API_KEY", env_key_name);
    let env_file = proxy_env_path()?;

    // 读取现有内容（如果存在）
    let mut lines: Vec<String> = if env_file.exists() {
        std::fs::read_to_string(&env_file)
            .unwrap_or_default()
            .lines()
            .map(|s| s.to_string())
            .collect()
    } else {
        vec!["# CodexProxy 自动生成的环境变量（由代理管理，请勿手动编辑）".to_string()]
    };

    // 检查是否已有该变量，若无则追加
    let export_line = format!("export {}=\"proxy\"", env_var_key);
    let already_exists = lines.iter().any(|l| l.starts_with(&format!("export {}=", env_var_key)));
    if !already_exists {
        lines.push(export_line);
        std::fs::write(&env_file, lines.join("\n") + "\n")
            .map_err(|e| format!("写入 proxy-env.sh 失败: {}", e))?;
    }

    // 确保 .zshrc 会 source proxy-env.sh
    let zshrc = zshrc_path()?;
    if zshrc.exists() {
        let zshrc_content = std::fs::read_to_string(&zshrc)
            .map_err(|e| format!("读取 .zshrc 失败: {}", e))?;
        let source_line = "[ -f ~/.codex/proxy-env.sh ] && source ~/.codex/proxy-env.sh";
        if !zshrc_content.contains("proxy-env.sh") {
            std::fs::write(&zshrc, format!("{}\n{}\n", zshrc_content.trim_end(), source_line))
                .map_err(|e| format!("更新 .zshrc 失败: {}", e))?;
        }
    }

    Ok(())
}

/// 同步代理配置到 Codex config.toml
/// - 更新顶层 model 和 model_provider
/// - 添加/更新 [model_providers.<name>]
/// - 添加/更新 [profiles.<name>]
pub fn sync_codex_config(name: &str, model: &str, port: u16) -> Result<(), String> {
    if name.is_empty() || model.is_empty() {
        return Ok(());
    }

    let key = sanitize_name(name);
    let config_path = codex_config_path()?;

    // 读取现有配置
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("读取 Codex 配置失败: {}", e))?;
    let mut doc = content.parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("解析 Codex 配置失败: {}", e))?;

    // 更新顶层 model 和 model_provider
    doc["model"] = value(model);
    doc["model_provider"] = value(&key);

    // 确保 model_providers 表存在
    if doc.get("model_providers").is_none() {
        doc["model_providers"] = Item::Table(Table::new());
    }

    // 在 model_providers 表中创建/获取子表（需要手动导航嵌套）
    let env_key_name = name.to_uppercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect::<String>();

    {
        let providers = doc["model_providers"].as_table_mut()
            .ok_or("model_providers 不是有效的表")?;

        if providers.get(&key).is_none() {
            providers.insert(&key, Item::Table(Table::new()));
        }

        let provider = providers.get_mut(&key).and_then(|i| i.as_table_mut())
            .ok_or("无法获取 provider 子表")?;

        provider.insert("name", value(format!("{} via Proxy", name)));
        provider.insert("base_url", value(format!("http://localhost:{}/v1", port)));
        provider.insert("requires_openai_auth", value(false));
        provider.insert("env_key", value(format!("{}_API_KEY", env_key_name)));
        provider.insert("wire_api", value("responses"));
    }

    // 确保 profiles 表存在
    if doc.get("profiles").is_none() {
        doc["profiles"] = Item::Table(Table::new());
    }

    {
        let profiles = doc["profiles"].as_table_mut()
            .ok_or("profiles 不是有效的表")?;

        if profiles.get(&key).is_none() {
            profiles.insert(&key, Item::Table(Table::new()));
        }

        let profile = profiles.get_mut(&key).and_then(|i| i.as_table_mut())
            .ok_or("无法获取 profile 子表")?;

        profile.insert("model", value(model));
        profile.insert("model_provider", value(&key));
    }

    // 写回 config.toml
    std::fs::write(&config_path, doc.to_string())
        .map_err(|e| format!("写入 Codex 配置失败: {}", e))?;

    // 写入 OS 级别环境变量文件（Codex 检查 env_key 时读取的是 OS 环境变量）
    set_proxy_env_var(&env_key_name)?;

    log::info!("已同步 Codex 配置: model={}, provider={}", model, key);
    Ok(())
}
