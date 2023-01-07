use super::*;
use ngrammatic::CorpusBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RedAlertVoiceConfig<ID: Eq + Hash> {
    pub target_words: Vec<String>,
    pub self_words: Vec<String>,
    pub aliases: HashMap<String, ID>,
    pub similarity_threshold: f32,
}

pub struct RedAlertVoiceSearchResult {
    pub word: String,
    pub real_word: String,
    pub total_similarity: f32,
}

impl<ID: Eq + Hash> RedAlertVoiceConfig<ID> {
    pub fn should_kick<'a, 'm: 'a>(
        &'m self,
        author_user_id: &'a ID,
        text: &String,
    ) -> HashMap<&'a ID, RedAlertVoiceSearchResult> {
        let similarity_threshold = self.similarity_threshold.min(1.0).max(0.0);
        let mut corpus = CorpusBuilder::new().finish();
        let text_words = text.split_ascii_whitespace();
        for text_word in text_words {
            corpus.add_text(text_word);
        }
        let check_text_contains = |query: &String| -> Option<(String, f32)> {
            let query_words: Vec<&str> = query.split_ascii_whitespace().collect();
            let single_word_threshold = similarity_threshold / (query_words.len() as f32);
            let mut real_query_words: Vec<String> = vec![];
            let mut total_similarity_sum: f32 = 0.0;
            for query_word in query_words {
                let mut search_result = corpus.search(query_word, single_word_threshold);
                if !(search_result.len() > 0) {
                    return None;
                }
                let first_search_result = search_result.remove(0);
                let real_query_word = first_search_result.text;
                let word_similarity = first_search_result.similarity;
                let letters_count_threshold = ((real_query_word.len() as f32)
                    * (1.0 - similarity_threshold))
                    .round() as usize;
                let real_letters_count = real_query_word.len();
                let letters_count = query_word.len();
                let letters_count_different =
                    real_letters_count.max(letters_count) - real_letters_count.min(letters_count);
                if !(letters_count_different <= letters_count_threshold) {
                    return None;
                }
                real_query_words.push(real_query_word);
                total_similarity_sum += word_similarity;
            }
            let total_similarity = total_similarity_sum / (real_query_words.len() as f32);
            if total_similarity >= similarity_threshold {
                let real_query = real_query_words.join(SPACE);
                if text.contains(&real_query) {
                    Some((real_query, total_similarity))
                } else {
                    None
                }
            } else {
                None
            }
        };
        let mut users_ids = HashMap::new();
        for self_word in &self.self_words {
            let Some((real_self_word, total_similarity)) = check_text_contains(&self_word) else {
                continue;
            };
            users_ids.insert(
                author_user_id,
                RedAlertVoiceSearchResult {
                    word: self_word.clone(),
                    real_word: real_self_word,
                    total_similarity,
                },
            );
            break;
        }
        for target_word in &self.target_words {
            let Some(_) = check_text_contains(target_word) else {
                continue;
            };
            for (name, user_id) in &self.aliases {
                let target_word_name = vec![target_word.to_owned(), name.to_owned()].join(SPACE);
                let Some((real_target_word_name, total_similarity)) = check_text_contains(&target_word_name) else {
                    continue;
                };
                users_ids.insert(
                    user_id,
                    RedAlertVoiceSearchResult {
                        word: target_word_name,
                        real_word: real_target_word_name,
                        total_similarity,
                    },
                );
            }
        }
        users_ids
    }
}
