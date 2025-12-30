use phf::phf_map;

/// Languages the UI is translated into. English is the default and the
/// fallback for any key missing from a non-English table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Lang {
    #[default]
    En,
    De,
    Zh,
}

impl Lang {
    /// BCP-47 code suitable for the `lang` attribute on `<html>`.
    pub(crate) fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
            Lang::Zh => "zh",
        }
    }

    /// Look up `key` in the current language, falling back to English and
    /// finally to the key itself if the key is unknown everywhere. Keys are
    /// expected to be string literals.
    pub(crate) fn t(self, key: &'static str) -> &'static str {
        let map: &phf::Map<&'static str, &'static str> = match self {
            Lang::En => &EN,
            Lang::De => &DE,
            Lang::Zh => &ZH,
        };

        map.get(key).or_else(|| EN.get(key)).copied().unwrap_or(key)
    }

    /// Look up `key` and substitute the `{0}` placeholder with `arg`'s
    /// `Display` representation.
    pub(crate) fn t_with(self, key: &'static str, arg: impl std::fmt::Display) -> String {
        self.t(key).replace("{0}", &arg.to_string())
    }
}

static EN: phf::Map<&'static str, &'static str> = phf_map! {
    "nav.home" => "home",
    "nav.upload" => "upload",
    "nav.delete" => "delete paste",
    "nav.download" => "download file",
    "nav.raw" => "display raw file",
    "nav.copy" => "copy to clipboard",
    "nav.qr" => "qr code",
    "nav.rendered" => "rendered view",
    "nav.source" => "source view",

    "theme.dark" => "dark mode",
    "theme.light" => "light mode",
    "theme.auto" => "auto mode",

    "index.placeholder.paste" => "paste, type, or drop a file here …",
    "index.drop" => "drop to load file",
    "index.label.title" => "title",
    "index.placeholder.title" => "untitled",
    "index.label.language" => "language",
    "index.aria.language" => "Language",
    "index.placeholder.filter" => "filter …",
    "index.label.expires" => "expires",
    "index.label.options" => "options",
    "index.toggle.burn" => "burn after reading",
    "index.toggle.burn.hint" => "delete on first view",
    "index.toggle.encrypt" => "encrypt",
    "index.toggle.encrypt.hint" => "password-protect the paste",
    "index.placeholder.password" => "password",
    "index.stat.lines" => "lines",
    "index.stat.chars" => "chars",
    "index.stat.bytes" => "bytes",
    "index.button.paste" => "Paste",
    "index.button.paste.label" => "paste",

    "paste.expires_in" => "expires in",
    "paste.toast.copied_content" => "Copied content",
    "paste.toast.copied_url" => "Copied URL",
    "paste.toast.burned" => "Content is burned and cannot be looked up again!",
    "paste.help.go_home" => "Go home",
    "paste.help.go_here" => "Go here",
    "paste.help.copy_url" => "Copy URL",
    "paste.help.copy_content" => "Copy content",
    "paste.help.download" => "Download",
    "paste.help.show_qr" => "Show QR code",
    "paste.help.toggle_wrap" => "Toggle line wrapping",
    "paste.help.toggle_rendered" => "Toggle rendered view",
    "paste.help.toggle_help" => "Toggle help",

    "password.show" => "show password",
    "password.hide" => "hide password",

    "stats.unit.kb" => "kb",
    "stats.unit.mb" => "mb",
    "stats.label.limit" => "limit",

    "burn.title" => "Burn after reading",
    "burn.body" => "Copy and send <a class=\"text-link\" href=\"{0}//{1}\">this link</a>. The recipient will be shown a confirmation prompt. The paste is deleted the moment they confirm.",

    "burn_confirm.body" => "This paste will be <strong>permanently deleted</strong> the moment it is revealed. You will not be able to view it again.",
    "burn_confirm.cancel" => "cancel",
    "burn_confirm.reveal" => "reveal",

    "encrypted.title" => "Encrypted paste",
    "encrypted.placeholder" => "password …",
    "encrypted.cancel" => "cancel",
    "encrypted.decrypt" => "decrypt",

    "error.title" => "Error 😢",
    "error.back" => "go back",

    "qr.label" => "qr code",
};

static DE: phf::Map<&'static str, &'static str> = phf_map! {
    "nav.home" => "Start",
    "nav.upload" => "Hochladen",
    "nav.delete" => "Paste löschen",
    "nav.download" => "Datei herunterladen",
    "nav.raw" => "Rohansicht",
    "nav.copy" => "In Zwischenablage kopieren",
    "nav.qr" => "QR-Code",
    "nav.rendered" => "Gerenderte Ansicht",
    "nav.source" => "Quelltext-Ansicht",

    "theme.dark" => "Dunkler Modus",
    "theme.light" => "Heller Modus",
    "theme.auto" => "Automatisch",

    "index.placeholder.paste" => "Text einfügen, tippen oder Datei hierher ziehen …",
    "index.drop" => "Datei hier ablegen",
    "index.label.title" => "Titel",
    "index.placeholder.title" => "ohne Titel",
    "index.label.language" => "Sprache",
    "index.aria.language" => "Sprache",
    "index.placeholder.filter" => "filtern …",
    "index.label.expires" => "Läuft ab",
    "index.label.options" => "Optionen",
    "index.toggle.burn" => "Nach Lesen vernichten",
    "index.toggle.burn.hint" => "Nach erstem Aufruf löschen",
    "index.toggle.encrypt" => "Verschlüsseln",
    "index.toggle.encrypt.hint" => "Paste mit Passwort schützen",
    "index.placeholder.password" => "Passwort",
    "index.stat.lines" => "Zeilen",
    "index.stat.chars" => "Zeichen",
    "index.stat.bytes" => "Bytes",
    "index.button.paste" => "Einfügen",
    "index.button.paste.label" => "einfügen",

    "paste.expires_in" => "läuft ab in",
    "paste.toast.copied_content" => "Inhalt kopiert",
    "paste.toast.copied_url" => "URL kopiert",
    "paste.toast.burned" => "Inhalt ist vernichtet und kann nicht mehr abgerufen werden!",
    "paste.help.go_home" => "Zur Startseite",
    "paste.help.go_here" => "Zu diesem Paste",
    "paste.help.copy_url" => "URL kopieren",
    "paste.help.copy_content" => "Inhalt kopieren",
    "paste.help.download" => "Herunterladen",
    "paste.help.show_qr" => "QR-Code anzeigen",
    "paste.help.toggle_wrap" => "Zeilenumbruch umschalten",
    "paste.help.toggle_rendered" => "Markdown Ansicht umschalten",
    "paste.help.toggle_help" => "Hilfe umschalten",

    "password.show" => "Passwort anzeigen",
    "password.hide" => "Passwort verbergen",

    "stats.unit.kb" => "kB",
    "stats.unit.mb" => "MB",
    "stats.label.limit" => "Limit",

    "burn.title" => "Nach Lesen vernichten",
    "burn.body" => "Kopiere und schicke <a class=\"text-link\" href=\"{0}/{1}\">diesen Link</a>. Dem Empfänger wird eine Bestätigungsaufforderung angezeigt und der Paste nach Bestätigung gelöscht.",

    "burn_confirm.body" => "Dieser Paste wird <strong>unwiderruflich gelöscht</strong>, sobald er angezeigt wird und kann danach nicht mehr eingesehen werden.",
    "burn_confirm.cancel" => "Abbrechen",
    "burn_confirm.reveal" => "Anzeigen",

    "encrypted.title" => "Verschlüsselter Paste",
    "encrypted.placeholder" => "Passwort …",
    "encrypted.cancel" => "Abbrechen",
    "encrypted.decrypt" => "Entschlüsseln",

    "error.title" => "Fehler 😢",
    "error.back" => "Zurück",

    "qr.label" => "QR-Code",
};

static ZH: phf::Map<&'static str, &'static str> = phf_map! {
    "nav.home" => "主页",
    "nav.upload" => "上传",
    "nav.delete" => "删除剪贴",
    "nav.download" => "下载文件",
    "nav.raw" => "显示原始内容",
    "nav.copy" => "复制到剪贴板",
    "nav.qr" => "二维码",
    "nav.rendered" => "渲染视图",
    "nav.source" => "源码视图",

    "theme.dark" => "深色模式",
    "theme.light" => "浅色模式",
    "theme.auto" => "自动模式",

    "index.placeholder.paste" => "在此处粘贴、输入或拖放文件…",
    "index.drop" => "拖放以加载文件",
    "index.label.title" => "标题",
    "index.placeholder.title" => "无标题",
    "index.label.language" => "语言",
    "index.aria.language" => "语言",
    "index.placeholder.filter" => "过滤…",
    "index.label.expires" => "过期时间",
    "index.label.options" => "选项",
    "index.toggle.burn" => "阅后即焚",
    "index.toggle.burn.hint" => "首次查看后删除",
    "index.toggle.encrypt" => "加密",
    "index.toggle.encrypt.hint" => "使用密码保护剪贴",
    "index.placeholder.password" => "密码",
    "index.stat.lines" => "行",
    "index.stat.chars" => "字符",
    "index.stat.bytes" => "字节",
    "index.button.paste" => "粘贴",
    "index.button.paste.label" => "粘贴",

    "paste.expires_in" => "过期于",
    "paste.toast.copied_content" => "已复制内容",
    "paste.toast.copied_url" => "已复制链接",
    "paste.toast.burned" => "内容已销毁，无法再次查看！",
    "paste.help.go_home" => "返回主页",
    "paste.help.go_here" => "返回此处",
    "paste.help.copy_url" => "复制链接",
    "paste.help.copy_content" => "复制内容",
    "paste.help.download" => "下载",
    "paste.help.show_qr" => "显示二维码",
    "paste.help.toggle_wrap" => "切换自动换行",
    "paste.help.toggle_rendered" => "切换渲染视图",
    "paste.help.toggle_help" => "切换帮助",

    "password.show" => "显示密码",
    "password.hide" => "隐藏密码",

    "stats.unit.kb" => "kb",
    "stats.unit.mb" => "mb",
    "stats.label.limit" => "限制",

    "burn.title" => "阅后即焚",
    "burn.body" => "复制并发送 <a class=\"text-link\" href=\"{0}/{1}\">此链接</a>。收件人将看到确认提示。在他们确认的那一刻，剪贴将被删除。",

    "burn_confirm.body" => "此剪贴在显示的那一刻将被 <strong>永久删除</strong>。您将无法再次查看它。",
    "burn_confirm.cancel" => "取消",
    "burn_confirm.reveal" => "显示",

    "encrypted.title" => "加密的剪贴",
    "encrypted.placeholder" => "密码…",
    "encrypted.cancel" => "取消",
    "encrypted.decrypt" => "解密",

    "error.title" => "错误 😢",
    "error.back" => "返回",

    "qr.label" => "二维码",
};

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;

    #[test]
    fn falls_back_to_english_for_missing_keys() {
        assert_eq!(Lang::De.t("nav.home"), "Start");
        assert_eq!(Lang::En.t("nav.home"), "home");
        // Unknown key returns the key itself.
        assert_eq!(Lang::De.t("does.not.exist"), "does.not.exist");
    }

    #[test]
    fn t_with_substitutes_placeholder() {
        let s = Lang::En.t_with("burn.body", "foobar", "abc123");
        assert!(s.contains("href=\"foobar/abc123\""));
    }

    #[test]
    fn translations_intersect() {
        for perms in [&DE, &EN, &ZH].iter().permutations(2) {
            let a = perms[0];
            let b = perms[1];

            for key in a.keys() {
                assert!(b.contains_key(key));
            }

            for key in b.keys() {
                assert!(a.contains_key(key));
            }
        }
    }
}
