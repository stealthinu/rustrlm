import unittest


class EvalMatchingTests(unittest.TestCase):
    def test_ground_truth_to_doc_ids_whitespace_substring(self):
        from rustrlm_client.eval.matching import ground_truth_to_doc_ids

        docs = [
            ("a", "Hello world.\n\nThis is a paragraph about taste and art."),
            ("b", "Other content."),
        ]
        gt = ["This is a paragraph about taste and art."]

        self.assertEqual(ground_truth_to_doc_ids(docs, gt), {"a"})

    def test_ground_truth_to_doc_ids_alnum_substring_handles_markdown(self):
        from rustrlm_client.eval.matching import ground_truth_to_doc_ids

        docs = [
            (
                "x",
                "> Supported comfortably, Newton was free.\n"
                "> To remain on, he had only to avoid the three unforgivable sins:\n"
                "> crime, heresy, and marriage.",
            ),
        ]
        gt = ["To remain on, he had only to avoid the three unforgivable sins: crime, heresy, and marriage."]

        self.assertEqual(ground_truth_to_doc_ids(docs, gt), {"x"})

    def test_hit_doc_id(self):
        from rustrlm_client.eval.matching import hit_doc_id

        gt_ids = {"a", "b"}
        retrieved = ["c", "b", "d"]
        self.assertTrue(hit_doc_id(retrieved, gt_ids))

        retrieved2 = ["c", "d"]
        self.assertFalse(hit_doc_id(retrieved2, gt_ids))

    def test_hit_text_relaxed(self):
        from rustrlm_client.eval.matching import hit_text_relaxed

        retrieved = [
            "The quick brown fox jumps over the lazy dog.",
            "Unrelated content.",
        ]
        gt = ["quick brown fox jumps over a lazy dog"]
        self.assertTrue(hit_text_relaxed(retrieved, gt))

        retrieved2 = ["xyz xyz xyz"]
        gt2 = ["abcdef"]
        self.assertFalse(hit_text_relaxed(retrieved2, gt2))


if __name__ == "__main__":
    unittest.main()
