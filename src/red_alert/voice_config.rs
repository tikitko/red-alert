use super::*;
use ngrammatic::CorpusBuilder;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RedAlertVoiceConfig<ID: Eq + Hash> {
    pub target_words: Vec<String>,
    pub self_words: Vec<String>,
    pub aliases: HashMap<String, ID>,
    pub similarity_threshold: f32,
}

impl<ID: Eq + Hash> RedAlertVoiceConfig<ID> {
    pub fn should_kick<'a, 'm: 'a>(
        &'m self,
        author_user_id: &'a ID,
        text: &String,
    ) -> HashSet<&'a ID> {
        let similarity_threshold = self.similarity_threshold.min(1.0).max(0.0);
        let mut corpus = CorpusBuilder::new().finish();
        let text_words = text.split_ascii_whitespace();
        for text_word in text_words {
            corpus.add_text(text_word);
        }
        let check_text_contains = |query: &String| {
            let query_words: Vec<&str> = query.split_ascii_whitespace().collect();
            let single_word_threshold = similarity_threshold / (query_words.len() as f32);
            let mut real_query_words: Vec<String> = vec![];
            let mut total_similarity_sum: f32 = 0.0;
            for query_word in query_words {
                let mut search_result = corpus.search(query_word, single_word_threshold);
                if !(search_result.len() > 0) {
                    return false;
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
                    return false;
                }
                real_query_words.push(real_query_word);
                total_similarity_sum += word_similarity;
            }
            let total_similarity = total_similarity_sum / (real_query_words.len() as f32);
            if total_similarity >= similarity_threshold {
                let real_query = real_query_words.join(SPACE);
                text.contains(&real_query)
            } else {
                false
            }
        };
        let mut users_ids = HashSet::new();
        for self_word in &self.self_words {
            if !check_text_contains(self_word) {
                continue;
            }
            users_ids.insert(author_user_id);
            break;
        }
        for target_word in &self.target_words {
            if !check_text_contains(target_word) {
                continue;
            }
            for (name, user_id) in &self.aliases {
                if !check_text_contains(name) {
                    continue;
                }
                users_ids.insert(user_id);
            }
        }
        users_ids
    }
}
