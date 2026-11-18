use super::{
    tokenize_words, AiEngine, EmbeddingPair, EmbeddingRagBenchReport, RagReport, RetrievedChunk,
};

impl AiEngine {
    // 9. RAG Inspector ------------------------------------------------------
    /// Assess answer grounding: a chunk is "used" when it shares a meaningful
    /// token overlap with the answer.
    pub fn inspect_rag(&self, chunks: &[RetrievedChunk], answer: &str) -> RagReport {
        let answer_tokens: std::collections::HashSet<String> =
            tokenize_words(answer).into_iter().collect();
        let mut used = Vec::new();
        let mut unused = Vec::new();
        for c in chunks {
            let chunk_tokens: std::collections::HashSet<String> =
                tokenize_words(&c.text).into_iter().collect();
            let overlap = answer_tokens.intersection(&chunk_tokens).count();
            // Require meaningful overlap relative to chunk size.
            let ratio = overlap as f64 / chunk_tokens.len().max(1) as f64;
            if overlap >= 3 || ratio >= 0.3 {
                used.push(c.id.clone());
            } else {
                unused.push(c.id.clone());
            }
        }
        let total = chunks.len().max(1);
        RagReport {
            grounded: !used.is_empty(),
            grounding_ratio: used.len() as f64 / total as f64,
            used_chunks: used,
            unused_chunks: unused,
        }
    }

    pub fn embedding_rag_bench(
        &self,
        pairs: &[EmbeddingPair],
        k: usize,
    ) -> EmbeddingRagBenchReport {
        let mut scored = pairs.to_vec();
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let topk = scored.iter().take(k.max(1)).collect::<Vec<_>>();
        let relevant_hits = topk.iter().filter(|p| p.relevant).count();
        let precision_at_k = relevant_hits as f64 / topk.len().max(1) as f64;
        let rel: Vec<f64> = pairs
            .iter()
            .filter(|p| p.relevant)
            .map(|p| p.score)
            .collect();
        let irrel: Vec<f64> = pairs
            .iter()
            .filter(|p| !p.relevant)
            .map(|p| p.score)
            .collect();
        let avg_relevant_score = if rel.is_empty() {
            0.0
        } else {
            rel.iter().sum::<f64>() / rel.len() as f64
        };
        let avg_irrelevant_score = if irrel.is_empty() {
            0.0
        } else {
            irrel.iter().sum::<f64>() / irrel.len() as f64
        };
        EmbeddingRagBenchReport {
            pairs: pairs.len(),
            precision_at_k,
            avg_relevant_score,
            avg_irrelevant_score,
            separation: avg_relevant_score - avg_irrelevant_score,
        }
    }
}
