"""Tests for analysis of Phase 4 records. Fixtures are hand-written, not measurements."""

from __future__ import annotations

import csv
import json
import math
import unittest
from pathlib import Path

import plot_results as analysis


def record(
    protocol: str = "GoBackN",
    reorder: float = 0.0,
    seed: int = 101,
    trial_id: int = 1,
    status: str = "SUCCESS",
    sha: bool = True,
    retransmissions: int = 0,
    offered: int = 100,
    reordered: int = 0,
    window: int | None = None,
) -> dict:
    if window is None:
        window = 1 if protocol == "StopAndWait" else 8
    return {
        "meta": {
            "experiment_id": f"fixture_{protocol}_{reorder}_{seed}",
            "protocol": protocol,
            "window_size": window,
            "chunk_size": 1400,
            "segment_count": 100,
            "seed": seed,
            "reverse_seed": seed + 1,
            "trial_id": trial_id,
        },
        "forward_config": {
            "configured_loss_rate": 0.0,
            "configured_reorder_rate": reorder,
            "configured_duplicate_rate": 0.0,
            "configured_corrupt_rate": 0.0,
            "base_delay_ms": 20,
            "jitter_ms": 0,
            "reorder_extra_ms": 50,
        },
        "reverse_config": {
            "configured_loss_rate": 0.0,
            "configured_reorder_rate": 0.0,
            "configured_duplicate_rate": 0.0,
            "configured_corrupt_rate": 0.0,
            "base_delay_ms": 20,
            "jitter_ms": 0,
            "reorder_extra_ms": 50,
        },
        "rto_config": {
            "rto_multiplier": 1.0,
            "initial_rto_ms": 1000.0,
            "min_rto_ms": 200.0,
            "max_rto_ms": 60000.0,
            "alpha": 0.125,
            "beta": 0.25,
            "k": 4.0,
        },
        "app": {
            "source_bytes": 140000,
            "delivered_unique_bytes": 140000 if status == "SUCCESS" else 0,
            "transfer_status": status,
            "sha256_match": sha,
            "source_sha256": "aa",
            "delivered_sha256": "aa" if sha else "bb",
        },
        "timing": {
            "duration_secs": 1.0 + retransmissions,
            "total_rtt_samples": 10,
            "karn_rejected_count": 0,
            "min_rtt_ms": 40.0,
            "max_rtt_ms": 40.0,
            "mean_rtt_ms": 40.0 + reorder,
            "final_srtt_ms": 40.0,
            "final_rttvar_ms": 1.0,
            "final_rto_ms": 200.0,
            "timeout_count": retransmissions,
            "backoff_count": retransmissions,
        },
        "protocol": {
            "data_sent": 100,
            "ack_sent": 100,
            "data_received": 100,
            "ack_received": 100,
            "data_retransmissions": retransmissions,
            "timeout_retransmissions": retransmissions,
            "premature_retransmissions": 0,
            "duplicate_data_received": 0,
            "duplicate_acks_received": 0,
        },
        "channel": {
            "offered_datagrams": offered,
            "dropped_datagrams": 0,
            "duplicated_datagrams": 0,
            "corrupted_datagrams": 0,
            "reordered_datagrams": reordered,
            "scheduled_deliveries": offered,
        },
        "reverse_channel": {
            "offered_datagrams": 100,
            "dropped_datagrams": 0,
            "duplicated_datagrams": 0,
            "corrupted_datagrams": 0,
            "reordered_datagrams": 0,
            "scheduled_deliveries": 100,
        },
        "goodput_bytes_sec": 1000.0,
        "retransmission_ratio": retransmissions / 100,
    }


class AnalysisTests(unittest.TestCase):
    def test_json_loading_and_missing_file(self) -> None:
        with self._temp() as raw:
            path = Path(raw) / "records.json"
            payload = [record()]
            path.write_text(json.dumps(payload), encoding="utf-8")
            loaded = analysis.load_records(path)
            self.assertEqual(len(loaded), 1)
            self.assertEqual(loaded[0]["meta"]["protocol"], "GoBackN")
        with self.assertRaises(analysis.AnalysisError) as caught:
            analysis.load_records(Path("/tmp/rdt-analysis-missing-does-not-exist.json"))
        self.assertIn("does not generate", str(caught.exception))

    def test_schema_validation_rejects_a_flat_record(self) -> None:
        with self.assertRaises(analysis.AnalysisError):
            analysis.validate_dataset([{"protocol": "GoBackN", "data_retransmissions": 1}])

    def test_primary_filter_reports_a_mismatch(self) -> None:
        outside = record(seed=7)
        self.assertIsNotNone(analysis.primary_mismatch(outside))
        self.assertIsNone(analysis.primary_mismatch(record()))

    def test_success_and_failure_classification(self) -> None:
        self.assertEqual(analysis.outcome(record()), "success")
        self.assertEqual(analysis.outcome(record(status="TIMEOUT", sha=False)), "timeout")
        self.assertEqual(analysis.outcome(record(status="INTEGRITY_FAIL", sha=False)), "integrity")
        bad_hash = record()
        bad_hash["app"]["sha256_match"] = False
        self.assertEqual(analysis.outcome(bad_hash), "integrity")

    def test_mean_std_sem_and_t_interval(self) -> None:
        values = [1.0, 2.0, 3.0, 4.0, 5.0]
        self.assertEqual(analysis.mean(values), 3.0)
        self.assertAlmostEqual(analysis.sample_std(values), math.sqrt(2.5))
        self.assertAlmostEqual(analysis.standard_error(values), math.sqrt(2.5) / math.sqrt(5))
        self.assertAlmostEqual(analysis.t_critical_95(4), 2.7764451051977987, places=6)
        low, high = analysis.confidence_interval_95(values)
        half = analysis.t_critical_95(4) * analysis.standard_error(values)
        self.assertAlmostEqual(low, 3.0 - half)
        self.assertAlmostEqual(high, 3.0 + half)

    def test_single_trial_has_no_interval(self) -> None:
        self.assertEqual(analysis.confidence_interval_95([4.0]), (None, None))
        self.assertIsNone(analysis.sample_std([4.0]))

    def test_grouping_and_failure_exclusion_from_the_mean(self) -> None:
        records = [
            record("GoBackN", 0.0, 101, 1, retransmissions=2),
            record("GoBackN", 0.0, 202, 2, retransmissions=4),
            record("GoBackN", 0.0, 303, 3, status="TIMEOUT", sha=False, retransmissions=99),
            record("SelectiveRepeat", 0.25, 101, 1, retransmissions=1, reordered=20),
        ]
        metrics, failures, mismatches = analysis.aggregate(records)
        gbn = [
            row
            for row in metrics
            if row["protocol"] == "GoBackN"
            and row["configured_reorder_rate"] == "0.00"
            and row["metric"] == "data_retransmissions"
        ][0]
        self.assertEqual(gbn["n"], 2)
        self.assertEqual(gbn["mean"], 3.0)
        failure = [
            row
            for row in failures
            if row["protocol"] == "GoBackN" and row["configured_reorder_rate"] == "0.00"
        ][0]
        self.assertEqual(failure["total_trials"], 3)
        self.assertEqual(failure["successful_trials"], 2)
        self.assertEqual(failure["timeout_failures"], 1)
        self.assertEqual(failure["failed_trials"], 1)
        self.assertEqual(mismatches, [])

    def test_configured_reorder_is_separate_from_measured_reorders(self) -> None:
        item = record(reorder=0.25, reordered=24, offered=100)
        self.assertEqual(analysis.configured_reorder(item), 0.25)
        self.assertAlmostEqual(analysis.actual_reorder_fraction(item), 0.24)
        self.assertIsNone(analysis.actual_reorder_fraction(record(offered=0, reordered=0)))

    def test_summary_csv_and_plots_from_a_tiny_fixture(self) -> None:
        records = [
            record("StopAndWait", 0.0, 101, 1, retransmissions=0),
            record("StopAndWait", 0.0, 202, 2, retransmissions=0),
            record("GoBackN", 0.25, 101, 1, retransmissions=5, reordered=10),
            record("GoBackN", 0.25, 202, 2, retransmissions=7, reordered=12),
            record("SelectiveRepeat", 0.25, 101, 1, retransmissions=1, reordered=10),
            record("SelectiveRepeat", 0.25, 202, 2, status="INTEGRITY_FAIL", sha=False, retransmissions=50),
        ]
        with self._temp() as raw:
            output = Path(raw) / "analysis"
            source = Path(raw) / "fixture.json"
            source.write_text(json.dumps(records), encoding="utf-8")
            loaded = analysis.load_records(source)
            result = analysis.analyze(loaded, output)
            with result["summary"].open(newline="", encoding="utf-8") as handle:
                rows = list(csv.DictReader(handle))
            self.assertEqual(
                list(rows[0]),
                ["protocol", "configured_reorder_rate", "metric", "n", "mean", "std", "sem", "ci95_low", "ci95_high"],
            )
            self.assertTrue(all(row["protocol"] in analysis.PROTOCOLS for row in rows))
            text = result["failures"].read_text(encoding="utf-8")
            self.assertIn("integrity_failures", text)
            for path in result["plots"]:
                svg = path.read_text(encoding="utf-8")
                self.assertIn("<svg", svg)
                self.assertIn("Configured forward reorder rate", svg)
                self.assertTrue(path.suffix == ".svg")
            diagnostic = (output / "actual_reorder_fraction_vs_configured.svg").read_text(encoding="utf-8")
            self.assertIn("Diagnostic:", diagnostic)

    def test_source_does_not_synthesize_trials(self) -> None:
        source = Path(analysis.__file__).read_text(encoding="utf-8")
        self.assertNotIn("generate_synthetic_trial", source)
        self.assertNotIn("run_experiments.main", source)

    def _temp(self):
        import tempfile

        return tempfile.TemporaryDirectory(prefix="rdt-analysis-")


if __name__ == "__main__":
    unittest.main()
