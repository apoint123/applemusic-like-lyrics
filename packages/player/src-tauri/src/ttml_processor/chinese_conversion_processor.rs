use std::sync::Arc;

use dashmap::DashMap;
use ferrous_opencc::OpenCC;
use std::sync::LazyLock;
use tracing::error;

use crate::ttml_processor::types::{ChineseConversionOptions, LyricLine};

static CONVERTER_CACHE: LazyLock<DashMap<String, Arc<OpenCC>>> = LazyLock::new(DashMap::new);

pub fn convert(text: &str, config_name: &str) -> String {
    if let Some(converter) = CONVERTER_CACHE.get(config_name) {
        return converter.convert(text);
    }

    match CONVERTER_CACHE
        .entry(config_name.to_string())
        .or_try_insert_with(|| {
            OpenCC::from_config_name(config_name)
                .map(Arc::new)
                .map_err(|e| {
                    error!("使用配置 '{}' 初始化 Opencc 转换器失败: {}", config_name, e);
                    e
                })
        }) {
        Ok(converter_ref) => converter_ref.value().convert(text),
        Err(_) => text.to_string(),
    }
}

#[derive(Debug, Default)]
pub struct ChineseConversionProcessor;

impl ChineseConversionProcessor {
    pub fn new() -> Self {
        Self
    }

    pub fn process(&self, lines: &mut [LyricLine], options: &ChineseConversionOptions) {
        let Some(config_name) = options.config_name.as_ref().filter(|s| !s.is_empty()) else {
            return;
        };

        for line in lines.iter_mut() {
            if !line.main_syllables.is_empty() {
                for syllable in &mut line.main_syllables {
                    syllable.text = convert(&syllable.text, config_name);
                }
            }

            if let Some(text) = &mut line.line_text {
                *text = convert(text, config_name);
            } else {
                if !line.main_syllables.is_empty() {
                    line.line_text = Some(
                        line.main_syllables
                            .iter()
                            .map(|s| s.text.as_str())
                            .collect(),
                    );
                }
            }

            for translation in &mut line.translations {
                translation.text = convert(&translation.text, config_name);
            }
        }
    }
}
