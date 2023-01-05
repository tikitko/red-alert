use fluent::{bundle::FluentBundle as OriginFluentBundle, *};

type FluentBundle = OriginFluentBundle<FluentResource, intl_memoizer::concurrent::IntlLangMemoizer>;

#[derive(Clone)]
pub struct L10n {
    bundle: std::sync::Arc<FluentBundle>,
}

impl L10n {
    pub fn load(lang_id_string: &str) -> Self {
        let lang_id: unic_langid::LanguageIdentifier = lang_id_string
            .parse()
            .expect("Failed to parse an LanguageIdentifier string.");
        let mut bundle = FluentBundle::new_concurrent(vec![lang_id]);
        let ftl_string = std::fs::read_to_string(lang_id_string.to_owned() + ".ftl")
            .expect("Failed to read FTL file.");
        let res = FluentResource::try_new(ftl_string).expect("Failed to parse an FTL string.");
        bundle
            .add_resource(res)
            .expect("Failed to add FTL resources to the bundle.");
        Self {
            bundle: std::sync::Arc::new(bundle),
        }
    }

    pub fn string<'a>(&'a self, msg_id: &'a str, args: FluentArgs<'a>) -> String {
        let mut errors = vec![];
        let msg = self
            .bundle
            .get_message(msg_id)
            .expect("Message doesn't exist.");
        let pattern = msg.value().expect("Message has no value.");
        let value = self
            .bundle
            .format_pattern(pattern, Some(&args), &mut errors);
        value.to_string()
    }
}
