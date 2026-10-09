"""Unit tests for catalogue-sweep analysis. They do not launch Rust."""

from __future__ import annotations

import unittest

import analyze_catalogue_sweeps as sweeps


def record(protocol: str, seed: int, window: int, loss: float, *, goodput: float, retx: float, duration: float, status: str = "SUCCESS") -> dict:
    return {
        "meta": {
            "experiment_id": f"{protocol}-{seed}-{window}-{loss}",
            "protocol": protocol,
            "window_size": window,
            "seed": seed,
            "trial_id": 1,
        },
        "forward_config": {
            "configured_loss_rate": loss,
            "configured_reorder_rate": 0.0,
        },
        "app": {
            "transfer_status": status,
            "sha256_match": status == "SUCCESS",
            "delivered_sha256": "abc",
        },
        "timing": {"timeout_count": 0, "duration_secs": duration},
        "protocol": {"data_retransmissions": retx, "premature_retransmissions": 0},
        "goodput_bytes_sec": goodput,
    }


class CatalogueSweepAnalysisTests(unittest.TestCase):
    def test_failed_trial_is_left_out_of_the_mean(self) -> None:
        records = [
            record("GoBackN", 101, 8, 0.0, goodput=10, retx=1, duration=1),
            record("GoBackN", 202, 8, 0.0, goodput=30, retx=3, duration=3),
            record("GoBackN", 303, 8, 0.0, goodput=999, retx=999, duration=999, status="TIMEOUT"),
        ]
        rows = sweeps.aggregate_rows("loss", records)
        goodput = [
            row
            for row in rows
            if row["protocol"] == "GoBackN" and row["loss_rate"] == "0.00" and row["metric"] == "goodput_bytes_sec"
        ]
        self.assertEqual(len(goodput), 1)
        self.assertEqual(goodput[0]["n"], 2)
        self.assertEqual(goodput[0]["mean"], 20)

    def test_paired_difference_is_gbn_minus_sr_on_the_same_seed(self) -> None:
        records = []
        for seed, gbn, sr in ((101, 5, 1), (202, 7, 2)):
            records.append(record("GoBackN", seed, 8, 0.0, goodput=1, retx=gbn, duration=1))
            records.append(record("SelectiveRepeat", seed, 8, 0.0, goodput=1, retx=sr, duration=1))
        rows = [
            row
            for row in sweeps.paired_rows("loss", records)
            if row["loss_rate"] == "0.00" and row["metric"] == "data_retransmissions"
        ]
        self.assertEqual(rows[0]["n"], 2)
        self.assertEqual(rows[0]["mean_difference"], 4.5)

    def test_regression_reports_a_mismatch_without_requiring_protocols_to_match(self) -> None:
        new_records = [
            record("GoBackN", 101, 8, 0.0, goodput=10, retx=0, duration=0.5),
            record("SelectiveRepeat", 101, 8, 0.0, goodput=99, retx=4, duration=1.5),
        ]
        primary = [
            record("GoBackN", 101, 8, 0.0, goodput=10, retx=0, duration=0.5),
            record("SelectiveRepeat", 101, 8, 0.0, goodput=99, retx=4, duration=1.5),
        ]
        rows = sweeps.regression_rows(new_records, primary)
        self.assertTrue(rows)
        self.assertTrue(all(row["match"] for row in rows))

        drifted = [
            record("GoBackN", 101, 8, 0.0, goodput=10, retx=0, duration=0.6),
            record("SelectiveRepeat", 101, 8, 0.0, goodput=99, retx=4, duration=1.5),
        ]
        drifted_rows = sweeps.regression_rows(drifted, primary)
        duration_rows = [row for row in drifted_rows if row["field"] == "timing.duration_secs" and row["protocol"] == "GoBackN"]
        self.assertEqual(duration_rows[0]["match"], False)


if __name__ == "__main__":
    unittest.main()
