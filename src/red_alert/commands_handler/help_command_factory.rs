use super::*;

pub(super) struct RedAlertHelpCommandFactory {
    pub(super) l10n: L10n,
}

impl HelpCommandFactory for RedAlertHelpCommandFactory {
    fn help_command(
        &self,
        commands_info: Vec<(String, HelpInfo)>,
    ) -> Box<dyn Command + Send + Sync + 'static> {
        Box::new(PrintTextCommand {
            prefix_anchor: self
                .l10n
                .string("help-command-prefix-anchor", fluent_args![]),
            help_info: None,
            text: commands_info
                .into_iter()
                .map(|(prefix_anchor, help_info)| {
                    vec![
                        if let Some(header_suffix) = help_info.header_suffix {
                            self.l10n.string(
                                "help-command-full-header",
                                fluent_args![
                                    "header" => prefix_anchor,
                                    "suffix" => header_suffix
                                ],
                            )
                        } else {
                            self.l10n.string(
                                "help-command-short-header",
                                fluent_args![
                                    "header" => prefix_anchor
                                ],
                            )
                        },
                        self.l10n.string(
                            "help-command-body",
                            fluent_args![
                                "body" => help_info.description
                            ],
                        ),
                    ]
                    .join(NEW_LINE)
                })
                .collect::<Vec<String>>()
                .join(NEW_LINE),
        })
    }
}
