use fluent::*;
use unic_langid::*;

pub struct L10n {
    bundle: FluentBundle<FluentResource>,
}

impl L10n {
    pub fn load(lang_id_string: &str) -> Self {
        let lang_id: LanguageIdentifier = lang_id_string
            .parse()
            .expect("Failed to parse an LanguageIdentifier string.");
        let mut bundle = FluentBundle::new(vec![lang_id]);
        let ftl_string = std::fs::read_to_string(lang_id_string.to_owned() + ".ftl")
            .expect("Failed to read FTL file.");
        let res = FluentResource::try_new(ftl_string).expect("Failed to parse an FTL string.");
        bundle
            .add_resource(res)
            .expect("Failed to add FTL resources to the bundle.");
        Self { bundle }
    }

    pub fn string<'a, V: Into<FluentValue<'a>>>(
        &'a self,
        msg_id: &'a str,
        args: Vec<(&'a str, V)>,
    ) -> String {
        let args = FluentArgs::<'a>::from_iter(args);
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
