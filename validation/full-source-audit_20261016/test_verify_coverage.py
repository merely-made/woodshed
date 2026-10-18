"""Negative controls for the audit ledger's inclusive range contract."""
import unittest

from verify_coverage import covers


class CoverageRanges(unittest.TestCase):
    def test_exact_once_or_overlapping_reads_cover_the_whole_file(self):
        self.assertTrue(covers([[1, 3], [4, 8]], 8))
        self.assertTrue(covers([[4, 8], [1, 5]], 8))
        self.assertTrue(covers([{'start': 1, 'end': 1}], 1))
        self.assertTrue(covers([], 0))

    def test_missing_first_middle_or_last_line_cannot_count_complete(self):
        for spans in [[[2, 8]], [[1, 3], [5, 8]], [[1, 7]], []]:
            with self.subTest(spans=spans):
                self.assertFalse(covers(spans, 8))

    def test_ranges_outside_the_file_or_reversed_are_rejected(self):
        for spans in [[[0, 8]], [[1, 9]], [[3, 2]]]:
            with self.subTest(spans=spans):
                self.assertFalse(covers(spans, 8))


if __name__ == '__main__':
    unittest.main()
