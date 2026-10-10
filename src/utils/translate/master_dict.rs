//! The vocabulary that test method names are translated through.
//!
//! One [`Entry`] per `BaseCase` method that a JSON scenario or a recorded test
//! can name, with the word for it in every [`Language`]. The table is the one
//! source of truth: the per-language modules ([`chinese`](super::chinese),
//! [`dutch`](super::dutch), ...) read from it, so a language can never lack a
//! word another has, and a word is added in one place.
//!
//! Translations are written as identifiers: a phrase is joined with `_`
//! (`double_click` is `doble_clic` in Spanish), so a translated name can be
//! used as a function name.
//!
//! Three older short names keep working through [`aliases`]: `type`, `assert`
//! and `close`, which stand for `type_text`, `assert_text` and `close_window`,
//! and `scroll`, which stands for `scroll_to`.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::utils::translate::{master_dict, Language};
//!
//! let entry = master_dict::entry("double_click").expect("a known action");
//! assert_eq!(entry.get(Language::Spanish), "doble_clic");
//! assert_eq!(entry.get(Language::English), "double_click");
//!
//! // An older short name resolves to its method.
//! assert_eq!(master_dict::entry("type").map(|e| e.en), Some("type_text"));
//! ```

use std::collections::HashMap;

use super::language::Language;

/// One action and its word in every supported language.
///
/// `en` is the name of the `BaseCase` method.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entry {
    /// English: the `BaseCase` method name.
    pub en: &'static str,
    /// Chinese.
    pub zh: &'static str,
    /// Dutch.
    pub nl: &'static str,
    /// French.
    pub fr: &'static str,
    /// Italian.
    pub it: &'static str,
    /// Japanese.
    pub ja: &'static str,
    /// Korean.
    pub ko: &'static str,
    /// Portuguese.
    pub pt: &'static str,
    /// Russian.
    pub ru: &'static str,
    /// Spanish.
    pub es: &'static str,
}

impl Entry {
    /// The word for this action in `language`.
    #[must_use]
    pub const fn get(&self, language: Language) -> &'static str {
        match language {
            Language::English => self.en,
            Language::Chinese => self.zh,
            Language::Dutch => self.nl,
            Language::French => self.fr,
            Language::Italian => self.it,
            Language::Japanese => self.ja,
            Language::Korean => self.ko,
            Language::Portuguese => self.pt,
            Language::Russian => self.ru,
            Language::Spanish => self.es,
        }
    }
}

#[allow(clippy::too_many_arguments)]
const fn row(
    en: &'static str,
    zh: &'static str,
    nl: &'static str,
    fr: &'static str,
    it: &'static str,
    ja: &'static str,
    ko: &'static str,
    pt: &'static str,
    ru: &'static str,
    es: &'static str,
) -> Entry {
    Entry {
        en,
        zh,
        nl,
        fr,
        it,
        ja,
        ko,
        pt,
        ru,
        es,
    }
}

/// Columns: en, zh, nl, fr, it, ja, ko, pt, ru, es.
#[rustfmt::skip]
const ENTRIES: &[Entry] = &[
    row("open", "打开", "openen", "ouvrir", "aprire", "開く", "열기", "abrir", "открыть", "abrir"),
    row("click", "点击", "klikken", "cliquer", "cliccare", "クリック", "클릭", "clicar", "кликнуть", "clic"),
    row("type_text", "输入", "typen", "saisir", "digitare", "入力", "입력", "digitar", "ввести", "escribir"),
    row("assert_text", "断言", "beweren", "vérifier", "verificare", "検証", "검증", "verificar", "проверить", "verificar"),
    row("assert_element", "断言元素", "element_beweren", "vérifier_élément", "verificare_elemento", "要素検証", "요소_검증", "verificar_elemento", "проверить_элемент", "verificar_elemento"),
    row("wait_for_text", "等待文本", "wachten_op_tekst", "attendre_texte", "attendere_testo", "テキスト待機", "텍스트_대기", "esperar_texto", "ждать_текст", "esperar_texto"),
    row("wait", "等待", "wachten", "attendre", "attendere", "待機", "대기", "esperar", "ждать", "esperar"),
    row("sleep", "暂停", "pauzeren", "pause", "pausa", "一時停止", "일시정지", "pausar", "пауза", "pausa"),
    row("hover", "悬停", "hoveren", "survoler", "passare", "ホバー", "호버", "pairar", "навести", "hover"),
    row("hover_and_click", "悬停并点击", "hoveren_en_klikken", "survoler_et_cliquer", "passare_e_cliccare", "ホバーしてクリック", "호버_후_클릭", "pairar_e_clicar", "навести_и_кликнуть", "hover_y_clic"),
    row("select", "选择", "selecteren", "sélectionner", "selezionare", "選択", "선택", "selecionar", "выбрать", "seleccionar"),
    row("select_option_by_text", "按文本选择", "selecteren_op_tekst", "sélectionner_par_texte", "selezionare_per_testo", "テキストで選択", "텍스트로_선택", "selecionar_por_texto", "выбрать_по_тексту", "seleccionar_por_texto"),
    row("select_option_by_value", "按值选择", "selecteren_op_waarde", "sélectionner_par_valeur", "selezionare_per_valore", "値で選択", "값으로_선택", "selecionar_por_valor", "выбрать_по_значению", "seleccionar_por_valor"),
    row("drag_and_drop", "拖放", "slepen_en_neerzetten", "glisser_déposer", "trascinare_e_rilasciare", "ドラッグアンドドロップ", "드래그_앤_드롭", "arrastar_e_soltar", "перетащить", "arrastrar_y_soltar"),
    row("switch_to_frame", "切换到框架", "overschakelen_naar_frame", "basculer_vers_cadre", "passare_al_frame", "フレームに切替", "프레임으로_전환", "mudar_para_frame", "переключиться_на_фрейм", "cambiar_a_marco"),
    row("switch_to_default_content", "切换到默认内容", "overschakelen_naar_standaard_inhoud", "basculer_vers_contenu_par_défaut", "passare_al_contenuto_predefinito", "既定コンテンツに切替", "기본_콘텐츠로_전환", "mudar_para_conteúdo_padrão", "переключиться_на_основной_контент", "cambiar_a_contenido_predeterminado"),
    row("switch_to_window", "切换到窗口", "overschakelen_naar_venster", "basculer_vers_fenêtre", "passare_alla_finestra", "ウィンドウに切替", "창으로_전환", "mudar_para_janela", "переключиться_на_окно", "cambiar_a_ventana"),
    row("switch_to_new_window", "切换到新窗口", "overschakelen_naar_nieuw_venster", "basculer_vers_nouvelle_fenêtre", "passare_alla_nuova_finestra", "新しいウィンドウに切替", "새_창으로_전환", "mudar_para_nova_janela", "переключиться_на_новое_окно", "cambiar_a_nueva_ventana"),
    row("clear", "清空", "wissen", "effacer", "cancellare", "クリア", "지우기", "limpar", "очистить", "limpiar"),
    row("click_link_text", "点击链接文本", "klik_linktekst", "cliquer_texte_du_lien", "cliccare_testo_collegamento", "リンクテキストをクリック", "링크_텍스트_클릭", "clicar_texto_do_link", "кликнуть_текст_ссылки", "clic_texto_del_enlace"),
    row("submit", "提交", "indienen", "soumettre", "inviare", "送信", "제출", "enviar", "отправить", "enviar"),
    row("accept_alert", "接受警报", "melding_accepteren", "accepter_alerte", "accettare_avviso", "アラートを承認", "알림_수락", "aceitar_alerta", "принять_оповещение", "aceptar_alerta"),
    row("dismiss_alert", "关闭警报", "melding_negeren", "rejeter_alerte", "ignorare_avviso", "アラートを閉じる", "알림_닫기", "dispensar_alerta", "отклонить_оповещение", "descartar_alerta"),
    row("type_alert_text", "输入警报文本", "meldingstekst_typen", "saisir_texte_alerte", "digitare_testo_avviso", "アラートテキスト入力", "알림_텍스트_입력", "digitar_texto_alerta", "ввести_текст_оповещения", "escribir_texto_alerta"),
    row("clear_local_storage", "清空本地存储", "lokale_opslag_wissen", "vider_stockage_local", "svuotare_archiviazione_locale", "ローカルストレージをクリア", "로컬_스토리지_지우기", "limpar_armazenamento_local", "очистить_локальное_хранилище", "limpiar_almacenamiento_local"),
    row("remove_local_storage_item", "删除本地存储项", "lokale_opslag_item_verwijderen", "supprimer_élément_stockage_local", "rimuovere_elemento_archiviazione_locale", "ローカルストレージ項目削除", "로컬_스토리지_항목_삭제", "remover_item_armazenamento_local", "удалить_элемент_локального_хранилища", "eliminar_elemento_almacenamiento_local"),
    row("set_local_storage_item", "设置本地存储项", "lokale_opslag_item_instellen", "définir_élément_stockage_local", "impostare_elemento_archiviazione_locale", "ローカルストレージ項目設定", "로컬_스토리지_항목_설정", "definir_item_armazenamento_local", "установить_элемент_локального_хранилища", "establecer_elemento_almacenamiento_local"),
    row("js_click", "JS点击", "js_klikken", "cliquer_js", "cliccare_js", "JSクリック", "JS_클릭", "clicar_js", "js_кликнуть", "clic_js"),
    row("js_type", "JS输入", "js_typen", "saisir_js", "digitare_js", "JS入力", "JS_입력", "digitar_js", "js_ввести", "escribir_js"),
    row("set_attribute", "设置属性", "attribuut_instellen", "définir_attribut", "impostare_attributo", "属性設定", "속성_설정", "definir_atributo", "установить_атрибут", "establecer_atributo"),
    row("remove_attribute", "删除属性", "attribuut_verwijderen", "supprimer_attribut", "rimuovere_attributo", "属性削除", "속성_삭제", "remover_atributo", "удалить_атрибут", "eliminar_atributo"),
    row("choose_file", "选择文件", "bestand_kiezen", "choisir_fichier", "scegliere_file", "ファイル選択", "파일_선택", "escolher_arquivo", "выбрать_файл", "elegir_archivo"),
    row("go_back", "返回", "terug", "retour", "indietro", "戻る", "뒤로", "voltar", "назад", "atrás"),
    row("go_forward", "前进", "verder", "avancer", "avanti", "進む", "앞으로", "avançar", "вперед", "adelante"),
    row("refresh", "刷新", "vernieuwen", "rafraîchir", "aggiornare", "更新", "새로고침", "atualizar", "обновить", "refrescar"),
    row("delete_all_cookies", "删除所有Cookie", "alle_cookies_verwijderen", "supprimer_tous_cookies", "eliminare_tutti_cookie", "すべてのCookie削除", "모든_쿠키_삭제", "excluir_todos_cookies", "удалить_все_cookie", "eliminar_todas_cookies"),
    row("double_click", "双击", "dubbelklikken", "double_cliquer", "doppio_clic", "ダブルクリック", "더블클릭", "clicar_duas_vezes", "двойной_клик", "doble_clic"),
    row("context_click", "右键点击", "rechtsklikken", "clic_droit", "clic_destro", "右クリック", "우클릭", "clicar_direito", "правый_клик", "clic_derecho"),
    row("maximize_window", "最大化窗口", "venster_maximaliseren", "agrandir_fenêtre", "massimizzare_finestra", "ウィンドウ最大化", "창_최대화", "maximizar_janela", "развернуть_окно", "maximizar_ventana"),
    row("close_window", "关闭", "sluiten", "fermer", "chiudere", "閉じる", "닫기", "fechar", "закрыть", "cerrar"),
    row("scroll_to", "滚动", "scrollen", "défiler", "scorrere", "スクロール", "스크롤", "rolar", "прокрутить", "desplazar"),
    row("scroll_to_bottom", "滚动到底部", "scrollen_naar_onderkant", "défiler_vers_bas", "scorrere_in_basso", "下端へスクロール", "맨_아래로_스크롤", "rolar_para_baixo", "прокрутить_вниз", "desplazar_al_final"),
    row("scroll_to_top", "滚动到顶部", "scrollen_naar_bovenkant", "défiler_vers_haut", "scorrere_in_alto", "上端へスクロール", "맨_위로_스크롤", "rolar_para_cima", "прокрутить_вверх", "desplazar_al_inicio"),
];

/// Short names from before the table covered every method, and the method
/// each stands for. They are accepted anywhere an action name is.
const ALIASES: &[(&str, &str)] = &[
    ("type", "type_text"),
    ("assert", "assert_text"),
    ("scroll", "scroll_to"),
    ("close", "close_window"),
];

/// Every action in the dictionary, in the order they are documented.
#[must_use]
pub fn entries() -> &'static [Entry] {
    ENTRIES
}

/// The short names that stand for a method, as `(alias, method)` pairs.
#[must_use]
pub fn aliases() -> &'static [(&'static str, &'static str)] {
    ALIASES
}

/// The entry for `action`: a `BaseCase` method name, or one of the
/// [`aliases`]. `None` for any other text.
#[must_use]
pub fn entry(action: &str) -> Option<&'static Entry> {
    let method = ALIASES
        .iter()
        .find(|(alias, _)| *alias == action)
        .map_or(action, |(_, method)| *method);
    ENTRIES.iter().find(|entry| entry.en == method)
}

/// The word for `action` in `language`, or `None` if the dictionary does not
/// know `action`.
///
/// English returns `action` as it was given, so an alias such as `type` stays
/// `type` rather than becoming `type_text`.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::translate::{master_dict, Language};
///
/// assert_eq!(master_dict::translate("click", Language::Chinese), Some("点击"));
/// assert_eq!(master_dict::translate("type", Language::English), Some("type"));
/// assert_eq!(master_dict::translate("no_such_action", Language::French), None);
/// ```
#[must_use]
pub fn translate(action: &str, language: Language) -> Option<&'static str> {
    let entry = entry(action)?;
    if language == Language::English {
        return Some(
            ALIASES
                .iter()
                .map(|(alias, _)| *alias)
                .find(|alias| *alias == action)
                .unwrap_or(entry.en),
        );
    }
    Some(entry.get(language))
}

/// Every name the dictionary accepts, mapped to the `BaseCase` method it
/// stands for: each method to itself, and each [alias](aliases) to its method.
#[must_use]
pub fn get_master_dict() -> HashMap<&'static str, &'static str> {
    ENTRIES
        .iter()
        .map(|entry| (entry.en, entry.en))
        .chain(ALIASES.iter().copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// The words the three-column tables had before the dictionary was one
    /// table, in the order en, zh, nl, fr, it, ja, ko, pt, ru, es. They must
    /// keep translating the same way.
    const LEGACY: &[(&str, [&str; 9])] = &[
        (
            "open",
            [
                "打开",
                "openen",
                "ouvrir",
                "aprire",
                "開く",
                "열기",
                "abrir",
                "открыть",
                "abrir",
            ],
        ),
        (
            "click",
            [
                "点击",
                "klikken",
                "cliquer",
                "cliccare",
                "クリック",
                "클릭",
                "clicar",
                "кликнуть",
                "clic",
            ],
        ),
        (
            "type",
            [
                "输入",
                "typen",
                "saisir",
                "digitare",
                "入力",
                "입력",
                "digitar",
                "ввести",
                "escribir",
            ],
        ),
        (
            "assert",
            [
                "断言",
                "beweren",
                "vérifier",
                "verificare",
                "検証",
                "검증",
                "verificar",
                "проверить",
                "verificar",
            ],
        ),
        (
            "wait",
            [
                "等待",
                "wachten",
                "attendre",
                "attendere",
                "待機",
                "대기",
                "esperar",
                "ждать",
                "esperar",
            ],
        ),
        (
            "hover",
            [
                "悬停",
                "hoveren",
                "survoler",
                "passare",
                "ホバー",
                "호버",
                "pairar",
                "навести",
                "hover",
            ],
        ),
        (
            "scroll",
            [
                "滚动",
                "scrollen",
                "défiler",
                "scorrere",
                "スクロール",
                "스크롤",
                "rolar",
                "прокрутить",
                "desplazar",
            ],
        ),
        (
            "refresh",
            [
                "刷新",
                "vernieuwen",
                "rafraîchir",
                "aggiornare",
                "更新",
                "새로고침",
                "atualizar",
                "обновить",
                "refrescar",
            ],
        ),
        (
            "go_back",
            [
                "返回",
                "terug",
                "retour",
                "indietro",
                "戻る",
                "뒤로",
                "voltar",
                "назад",
                "atrás",
            ],
        ),
        (
            "go_forward",
            [
                "前进",
                "verder",
                "avancer",
                "avanti",
                "進む",
                "앞으로",
                "avançar",
                "вперед",
                "adelante",
            ],
        ),
        (
            "close",
            [
                "关闭",
                "sluiten",
                "fermer",
                "chiudere",
                "閉じる",
                "닫기",
                "fechar",
                "закрыть",
                "cerrar",
            ],
        ),
        (
            "submit",
            [
                "提交",
                "indienen",
                "soumettre",
                "inviare",
                "送信",
                "제출",
                "enviar",
                "отправить",
                "enviar",
            ],
        ),
        (
            "clear",
            [
                "清空",
                "wissen",
                "effacer",
                "cancellare",
                "クリア",
                "지우기",
                "limpar",
                "очистить",
                "limpiar",
            ],
        ),
        (
            "select",
            [
                "选择",
                "selecteren",
                "sélectionner",
                "selezionare",
                "選択",
                "선택",
                "selecionar",
                "выбрать",
                "seleccionar",
            ],
        ),
    ];

    const NON_ENGLISH: [Language; 9] = [
        Language::Chinese,
        Language::Dutch,
        Language::French,
        Language::Italian,
        Language::Japanese,
        Language::Korean,
        Language::Portuguese,
        Language::Russian,
        Language::Spanish,
    ];

    #[test]
    fn no_action_appears_twice() {
        let mut seen = HashSet::new();
        for entry in entries() {
            assert!(seen.insert(entry.en), "{} is listed twice", entry.en);
        }
    }

    #[test]
    fn every_language_has_a_word_for_every_action() {
        for entry in entries() {
            for language in Language::ALL {
                let word = entry.get(language);
                assert!(
                    !word.trim().is_empty(),
                    "{} has no {language} word",
                    entry.en
                );
                assert_eq!(word, word.trim(), "{} ({language}) has padding", entry.en);
            }
        }
    }

    #[test]
    fn translations_can_be_used_as_identifiers() {
        for entry in entries() {
            for language in Language::ALL {
                let word = entry.get(language);
                assert!(
                    word.chars().all(|c| c.is_alphanumeric() || c == '_'),
                    "{} ({language}): {word:?} has a character that is not allowed in a name",
                    entry.en
                );
                assert!(
                    !word.starts_with(|c: char| c.is_ascii_digit()),
                    "{} ({language}): {word:?} starts with a digit",
                    entry.en
                );
            }
        }
    }

    #[test]
    fn within_a_language_no_two_actions_share_a_word() {
        for language in Language::ALL {
            let mut seen = HashSet::new();
            for entry in entries() {
                let word = entry.get(language);
                assert!(
                    seen.insert(word),
                    "{language}: {word:?} translates more than one action (one is {})",
                    entry.en
                );
            }
        }
    }

    #[test]
    fn english_is_the_method_name() {
        for entry in entries() {
            assert_eq!(entry.get(Language::English), entry.en);
            assert_eq!(translate(entry.en, Language::English), Some(entry.en));
        }
    }

    #[test]
    fn a_translation_differs_from_the_english_name() {
        // Spanish keeps `hover`, as developers there do; every other row must
        // really be translated.
        for entry in entries() {
            for language in NON_ENGLISH {
                if entry.en == "hover" && language == Language::Spanish {
                    continue;
                }
                assert_ne!(
                    entry.get(language),
                    entry.en,
                    "{} was not translated to {language}",
                    entry.en
                );
            }
        }
    }

    #[test]
    fn the_words_from_before_the_merge_are_unchanged() {
        for (name, words) in LEGACY {
            assert!(entry(name).is_some(), "{name} is no longer known");
            for (language, expected) in NON_ENGLISH.iter().zip(words) {
                assert_eq!(
                    translate(name, *language),
                    Some(*expected),
                    "{name} in {language}"
                );
            }
            assert_eq!(translate(name, Language::English), Some(*name));
        }
    }

    #[test]
    fn an_alias_resolves_to_a_real_method() {
        for (alias, method) in aliases() {
            assert!(
                entries().iter().all(|entry| entry.en != *alias),
                "{alias} is both an alias and a method"
            );
            assert_eq!(entry(alias).map(|entry| entry.en), Some(*method));
        }
    }

    #[test]
    fn unknown_text_has_no_entry() {
        assert!(entry("").is_none());
        assert!(entry("Click").is_none(), "names are case sensitive");
        assert!(entry("click ").is_none(), "names are not trimmed");
        assert_eq!(translate("custom", Language::Spanish), None);
    }

    #[test]
    fn the_master_dict_lists_every_accepted_name() {
        let dict = get_master_dict();
        assert_eq!(dict.len(), entries().len() + aliases().len());
        assert_eq!(dict.get("click"), Some(&"click"));
        assert_eq!(dict.get("type"), Some(&"type_text"));
    }
}
