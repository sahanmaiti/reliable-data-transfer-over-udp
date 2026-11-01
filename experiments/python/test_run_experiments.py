"""Stdlib tests for the real experiment runner. They do not launch the Rust matrix."""

from __future__ import annotations

import csv
import io
import json
import subprocess
import unittest
from pathlib import Path

import run_experiments as runner


def sample_record(trial: runner.TrialSpec) -> dict:
    return {
        "meta": {
            "experiment_id": trial.experiment_id,
            "protocol": trial.protocol,
            "window_size": trial.window,
            "chunk_size": trial.chunk_size,
            "segment_count": 2,
            "seed": trial.seed,
            "reverse_seed": trial.seed + 1,
            "trial_id": trial.trial_id,
        },
        "forward_config": {
            "configured_loss_rate": trial.loss_rate,
            "configured_reorder_rate": trial.reorder_rate,
            "configured_duplicate_rate": trial.duplicate_rate,
            "configured_corrupt_rate": trial.corrupt_rate,
            "base_delay_ms": trial.base_delay_ms,
            "jitter_ms": trial.jitter_ms,
            "reorder_extra_ms": trial.reorder_extra_ms,
        },
        "reverse_config": {
            "configured_loss_rate": 0.0,
            "configured_reorder_rate": 0.0,
            "configured_duplicate_rate": 0.0,
            "configured_corrupt_rate": 0.0,
            "base_delay_ms": trial.base_delay_ms,
            "jitter_ms": trial.jitter_ms,
            "reorder_extra_ms": trial.reorder_extra_ms,
        },
        "rto_config": {
            "rto_multiplier": trial.rto_multiplier,
            "initial_rto_ms": float(trial.initial_rto_ms),
            "min_rto_ms": float(trial.min_rto_ms),
            "max_rto_ms": float(trial.max_rto_ms),
            "alpha": 0.125,
            "beta": 0.25,
            "k": 4.0,
        },
        "app": {
            "source_bytes": 140000,
            "delivered_unique_bytes": 140000,
            "transfer_status": "SUCCESS",
            "sha256_match": True,
            "source_sha256": "abc",
            "delivered_sha256": "abc",
        },
        "timing": {
            "duration_secs": 1.5,
            "total_rtt_samples": 10,
            "karn_rejected_count": 0,
            "min_rtt_ms": 40.0,
            "max_rtt_ms": 90.0,
            "mean_rtt_ms": 50.0,
            "final_srtt_ms": 48.0,
            "final_rttvar_ms": 8.0,
            "final_rto_ms": 200.0,
            "timeout_count": 0,
            "backoff_count": 0,
        },
        "protocol": {
            "data_sent": 100,
            "ack_sent": 100,
            "data_received": 100,
            "ack_received": 100,
            "data_retransmissions": 3,
            "timeout_retransmissions": 3,
            "premature_retransmissions": 1,
            "duplicate_data_received": 0,
            "duplicate_acks_received": 0,
        },
        "channel": {
            "offered_datagrams": 103,
            "dropped_datagrams": 0,
            "duplicated_datagrams": 0,
            "corrupted_datagrams": 0,
            "reordered_datagrams": 25,
            "scheduled_deliveries": 103,
        },
        "reverse_channel": {
            "offered_datagrams": 100,
            "dropped_datagrams": 0,
            "duplicated_datagrams": 0,
            "corrupted_datagrams": 0,
            "reordered_datagrams": 0,
            "scheduled_deliveries": 100,
        },
        "goodput_bytes_sec": 93333.333,
        "retransmission_ratio": 0.03,
    }


class RunnerTests(unittest.TestCase):
    def test_trial_ids_are_deterministic(self) -> None:
        study = runner.primary_reordering_study()
        first = runner.expand_trials(study)
        second = runner.expand_trials(study)
        self.assertEqual([trial.experiment_id for trial in first], [trial.experiment_id for trial in second])
        self.assertEqual(first[0].experiment_id, "primary_reordering_StopAndWait_r0.00_s101_t1")
        matched = [
            trial.trial_id
            for trial in first
            if trial.seed == 202 and trial.reorder_rate == 0.25
        ]
        self.assertEqual(matched, [2, 2, 2])

    def test_configuration_expansion_is_the_documented_matrix(self) -> None:
        trials = runner.expand_trials(runner.primary_reordering_study())
        self.assertEqual(len(trials), 3 * 6 * 5)
        self.assertEqual({trial.loss_rate for trial in trials}, {0.0})
        self.assertEqual({trial.window for trial in trials if trial.protocol == "StopAndWait"}, {1})
        self.assertEqual(
            {trial.window for trial in trials if trial.protocol != "StopAndWait"},
            {8},
        )
        filtered = runner.expand_trials(
            runner.primary_reordering_study(),
            protocols=["SelectiveRepeat"],
            reorder_rates=[0.25],
            seeds=[101],
        )
        self.assertEqual(len(filtered), 1)
        self.assertEqual(filtered[0].trial_id, 1)
        self.assertEqual(filtered[0].reorder_rate, 0.25)

    def test_command_is_an_argument_vector(self) -> None:
        trial = runner.expand_trials(
            runner.primary_reordering_study(),
            protocols=["GoBackN"],
            reorder_rates=[0.10],
            seeds=[303],
        )[0]
        command = runner.build_command(Path("/tmp/reliable_udp"), trial, Path("/tmp/out.json"))
        self.assertIsInstance(command, list)
        self.assertEqual(command[1], "run-experiment")
        self.assertIn("--experiment-id", command)
        self.assertIn(trial.experiment_id, command)
        self.assertIn("--seed", command)
        self.assertIn("303", command)
        self.assertIn("--window", command)
        self.assertIn("8", command)
        self.assertIn("--reorder-extra-ms", command)
        self.assertIn("50", command)
        self.assertNotIn("--shell", command)
        joined = " ".join(command)
        self.assertNotIn("&&", joined)

    def test_json_validation_rejects_a_failed_transfer(self) -> None:
        trial = runner.expand_trials(runner.primary_reordering_study(), max_trials=1)[0]
        record = sample_record(trial)
        runner.validate_record(record, trial)
        record["app"]["transfer_status"] = "TIMEOUT"
        record["app"]["sha256_match"] = False
        with self.assertRaises(runner.TrialFailed):
            runner.validate_record(record, trial)

    def test_flattening_matches_the_rust_header(self) -> None:
        self.assertEqual(list(runner.CSV_COLUMNS), runner.RUST_CSV_HEADER.split(","))
        self.assertEqual(len(runner.CSV_COLUMNS), len(runner.FLATTEN_PATHS))
        trial = runner.expand_trials(runner.primary_reordering_study(), max_trials=1)[0]
        flat = runner.flatten_record(sample_record(trial))
        self.assertEqual(list(flat), list(runner.CSV_COLUMNS))
        self.assertEqual(flat["data_retransmissions"], "3")
        self.assertEqual(flat["sha256_match"], "true")
        self.assertEqual(flat["reverse_reordered_datagrams"], "0")
        self.assertEqual(flat["premature_retransmissions"], "1")
        self.assertEqual(flat["goodput_bytes_sec"], "93333.333")

    def test_csv_round_trip_quotes_commas(self) -> None:
        trial = runner.expand_trials(runner.primary_reordering_study(), max_trials=1)[0]
        record = sample_record(trial)
        record["meta"]["experiment_id"] = "id,with,commas"
        buffer = io.StringIO()
        writer = csv.DictWriter(buffer, fieldnames=list(runner.CSV_COLUMNS))
        writer.writeheader()
        writer.writerow(runner.flatten_record(record))
        rows = list(csv.reader(io.StringIO(buffer.getvalue())))
        self.assertEqual(len(rows), 2)
        self.assertEqual(len(rows[0]), len(rows[1]))
        self.assertEqual(rows[1][0], "id,with,commas")

    def test_failed_subprocess_stops_without_a_dataset(self) -> None:
        trial = runner.expand_trials(runner.primary_reordering_study(), max_trials=1)[0]

        def execute(command: list[str]) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess(command, 2, stdout="partial", stderr="channel exploded")

        with self.assertRaises(runner.TrialFailed) as caught:
            runner.run_one_trial(Path("reliable_udp"), trial, Path("/tmp/unused-rdt-trials"), execute)
        self.assertIn("channel exploded", str(caught.exception))
        self.assertIn(trial.experiment_id, str(caught.exception))

    def test_successful_subprocess_reads_the_json_file(self) -> None:
        study = runner.primary_reordering_study()
        trials = runner.expand_trials(study, protocols=["SelectiveRepeat"], reorder_rates=[0.0], seeds=[101])
        self.assertEqual(len(trials), 1)

        def execute(command: list[str]) -> subprocess.CompletedProcess[str]:
            json_out = Path(command[command.index("--json-out") + 1])
            json_out.write_text(json.dumps(sample_record(trials[0])), encoding="utf-8")
            return subprocess.CompletedProcess(command, 0, stdout="Status: SUCCESS", stderr="")

        with self._temp_dir() as raw_dir:
            output_dir = Path(raw_dir)
            json_path, csv_path = runner.run_trials(trials, Path("reliable_udp"), output_dir, execute)
            stored = json.loads(json_path.read_text(encoding="utf-8"))
            self.assertEqual(stored[0]["protocol"]["data_retransmissions"], 3)
            self.assertEqual(stored[0]["app"]["sha256_match"], True)
            text = csv_path.read_text(encoding="utf-8")
            self.assertTrue(text.startswith(runner.RUST_CSV_HEADER))
            self.assertIn("primary_reordering_SelectiveRepeat_r0.00_s101_t1", text)

    def test_module_does_not_synthesize_measurements(self) -> None:
        source = Path(runner.__file__).read_text(encoding="utf-8")
        self.assertNotIn("generate_synthetic_trial", source)
        self.assertNotIn("random.seed", source)
        self.assertNotIn("gbn_cascade", source)
        self.assertFalse(hasattr(runner, "generate_synthetic_trial"))

    def test_primary_configuration_remains_unchanged(self) -> None:
        study = runner.primary_reordering_study()
        self.assertEqual(study.name, "primary_reordering")
        self.assertEqual(study.loss_rate, 0.0)
        self.assertEqual(study.reorder_rates, (0.0, 0.05, 0.10, 0.15, 0.20, 0.25))
        self.assertEqual(study.rto_multipliers, (1.0,))
        self.assertEqual(study.rto_multiplier, 1.0)
        self.assertEqual(study.min_rto_ms, 200)
        self.assertEqual(study.max_rto_ms, 60000)
        self.assertEqual(study.initial_rto_ms, 1000)
        self.assertEqual(study.seeds, (101, 202, 303, 404, 505))
        self.assertEqual(study.chunk_size, 1400)
        self.assertEqual(study.window_size, 8)
        self.assertEqual(study.loss_rates, ())
        self.assertEqual(study.window_sizes, ())
        self.assertEqual(study.base_delay_ms, 20)
        self.assertEqual(study.jitter_ms, 0)
        self.assertEqual(study.reorder_extra_ms, 50)
        self.assertEqual(len(runner.expand_trials(study)), 90)

    def test_e4_configuration_expansion_is_45_trials(self) -> None:
        study = runner.rto_sensitivity_study()
        trials = runner.expand_trials(study)
        self.assertEqual(len(trials), 3 * 3 * 5)
        self.assertEqual({trial.protocol for trial in trials}, {"StopAndWait", "GoBackN", "SelectiveRepeat"})
        self.assertEqual({trial.rto_multiplier for trial in trials}, {0.5, 1.0, 3.0})
        self.assertEqual({trial.loss_rate for trial in trials}, {0.05})
        self.assertEqual({trial.reorder_rate for trial in trials}, {0.0})
        self.assertEqual({trial.min_rto_ms for trial in trials}, {10})
        self.assertEqual({trial.max_rto_ms for trial in trials}, {60000})
        self.assertEqual({trial.initial_rto_ms for trial in trials}, {1000})
        self.assertEqual({trial.window for trial in trials if trial.protocol == "StopAndWait"}, {1})
        self.assertEqual({trial.window for trial in trials if trial.protocol != "StopAndWait"}, {8})
        self.assertEqual(sum(1 for t in trials if t.protocol == "StopAndWait"), 15)
        self.assertEqual(sum(1 for t in trials if t.rto_multiplier == 1.0), 15)
        self.assertEqual(sum(1 for t in trials if t.seed == 101), 9)

    def test_e4_experiment_ids_are_stable(self) -> None:
        study = runner.rto_sensitivity_study()
        first = runner.expand_trials(study)
        second = runner.expand_trials(study)
        self.assertEqual([t.experiment_id for t in first], [t.experiment_id for t in second])
        self.assertEqual(
            first[0].experiment_id,
            "rto_sensitivity_StopAndWait_m0.50_s101_t1",
        )
        matched = [
            t.experiment_id
            for t in first
            if t.protocol == "GoBackN" and t.rto_multiplier == 3.0 and t.seed == 505
        ]
        self.assertEqual(matched, ["rto_sensitivity_GoBackN_m3.00_s505_t5"])
        self.assertTrue(all(not t.experiment_id.count("-") > 4 for t in first))  # no UUID-like ids
        self.assertTrue(all("rto_sensitivity_" in t.experiment_id for t in first))

    def test_e4_output_directory_is_never_results_raw(self) -> None:
        self.assertEqual(
            runner.DEFAULT_RTO_OUTPUT_DIR,
            runner.REPO_ROOT / "results" / "rto_sensitivity",
        )
        self.assertNotEqual(runner.DEFAULT_RTO_OUTPUT_DIR, runner.DEFAULT_PRIMARY_OUTPUT_DIR)
        with self.assertRaises(runner.TrialFailed):
            runner.assert_output_dir_allowed(
                "rto_sensitivity",
                runner.DEFAULT_PRIMARY_OUTPUT_DIR,
            )
        with self.assertRaises(runner.TrialFailed):
            runner.assert_output_dir_allowed(
                "rto_sensitivity",
                runner.DEFAULT_PRIMARY_OUTPUT_DIR / "trials",
            )
        runner.assert_output_dir_allowed("rto_sensitivity", runner.DEFAULT_RTO_OUTPUT_DIR)

    def test_e4_json_identity_validation(self) -> None:
        trial = runner.expand_trials(
            runner.rto_sensitivity_study(),
            protocols=["SelectiveRepeat"],
            multipliers=[0.5],
            seeds=[202],
        )[0]
        self.assertEqual(trial.rto_multiplier, 0.5)
        self.assertEqual(trial.min_rto_ms, 10)
        self.assertEqual(trial.loss_rate, 0.05)
        record = sample_record(trial)
        runner.validate_record(record, trial)
        record["rto_config"]["rto_multiplier"] = 1.0
        with self.assertRaises(runner.TrialFailed):
            runner.validate_record(record, trial)
        record = sample_record(trial)
        record["rto_config"]["min_rto_ms"] = 200.0
        with self.assertRaises(runner.TrialFailed):
            runner.validate_record(record, trial)
        record = sample_record(trial)
        record["forward_config"]["configured_loss_rate"] = 0.0
        with self.assertRaises(runner.TrialFailed):
            runner.validate_record(record, trial)
        record = sample_record(trial)
        record["forward_config"]["configured_reorder_rate"] = 0.10
        with self.assertRaises(runner.TrialFailed):
            runner.validate_record(record, trial)

    def test_study_selector_is_required(self) -> None:
        with self.assertRaises(SystemExit):
            runner.parse_args([])
        args = runner.parse_args(["--study", "rto", "--multiplier", "0.5", "--seed", "101"])
        self.assertEqual(args.study, "rto")
        self.assertEqual(args.multiplier, [0.5])

    def test_window_study_is_65_trials_and_stop_and_wait_stays_at_one(self) -> None:
        study = runner.window_goodput_study()
        trials = runner.expand_trials(study)
        self.assertEqual(len(trials), 65)
        self.assertEqual(study.loss_rate, 0.0)
        self.assertEqual(study.reorder_rates, (0.0,))
        self.assertEqual(study.min_rto_ms, 200)
        self.assertEqual(study.window_sizes, (1, 2, 4, 8, 16, 32))
        sw = [t for t in trials if t.protocol == "StopAndWait"]
        self.assertEqual(len(sw), 5)
        self.assertEqual({t.window for t in sw}, {1})
        self.assertEqual({t.loss_rate for t in trials}, {0.0})
        self.assertEqual({t.reorder_rate for t in trials}, {0.0})
        self.assertEqual({t.rto_multiplier for t in trials}, {1.0})
        pipelined = [t for t in trials if t.protocol != "StopAndWait"]
        self.assertEqual(len(pipelined), 60)
        self.assertEqual({t.window for t in pipelined}, {1, 2, 4, 8, 16, 32})
        self.assertEqual(trials[0].experiment_id, "window_goodput_StopAndWait_w1_s101_t1")
        matched = [
            t.experiment_id
            for t in trials
            if t.protocol == "GoBackN" and t.window == 32 and t.seed == 505
        ]
        self.assertEqual(matched, ["window_goodput_GoBackN_w32_s505_t5"])
        forced = runner._make_trial(
            study,
            experiment_id_value="ignored",
            protocol="StopAndWait",
            seed=101,
            trial_id=1,
            reorder_rate=0.0,
            rto_multiplier=1.0,
            window=32,
        )
        self.assertEqual(forced.window, 1)

    def test_loss_study_is_90_trials(self) -> None:
        study = runner.loss_goodput_study()
        trials = runner.expand_trials(study)
        self.assertEqual(len(trials), 90)
        self.assertEqual(study.loss_rates, (0.0, 0.05, 0.10, 0.15, 0.20, 0.25))
        self.assertEqual(study.min_rto_ms, 200)
        self.assertEqual({t.loss_rate for t in trials}, {0.0, 0.05, 0.10, 0.15, 0.20, 0.25})
        self.assertEqual({t.reorder_rate for t in trials}, {0.0})
        self.assertEqual({t.rto_multiplier for t in trials}, {1.0})
        self.assertEqual({t.window for t in trials if t.protocol == "StopAndWait"}, {1})
        self.assertEqual({t.window for t in trials if t.protocol != "StopAndWait"}, {8})
        self.assertEqual(trials[0].experiment_id, "loss_goodput_StopAndWait_l0.00_s101_t1")
        matched = [
            t.experiment_id
            for t in trials
            if t.protocol == "GoBackN" and t.loss_rate == 0.25 and t.seed == 505
        ]
        self.assertEqual(matched, ["loss_goodput_GoBackN_l0.25_s505_t5"])
        self.assertEqual(
            [t.experiment_id for t in trials],
            [t.experiment_id for t in runner.expand_trials(study)],
        )

    def test_new_studies_refuse_frozen_result_directories(self) -> None:
        frozen = (
            runner.DEFAULT_PRIMARY_OUTPUT_DIR,
            runner.DEFAULT_PRIMARY_OUTPUT_DIR / "trials",
            runner.DEFAULT_RTO_OUTPUT_DIR,
            runner.DEFAULT_RTO_OUTPUT_DIR / "analysis",
        )
        for study_name, own_dir in (
            ("window_goodput", runner.DEFAULT_WINDOW_OUTPUT_DIR),
            ("loss_goodput", runner.DEFAULT_LOSS_OUTPUT_DIR),
        ):
            runner.assert_output_dir_allowed(study_name, own_dir)
            for path in frozen:
                with self.assertRaises(runner.TrialFailed):
                    runner.assert_output_dir_allowed(study_name, path)
        with self.assertRaises(runner.TrialFailed):
            runner.assert_output_dir_allowed("window_goodput", runner.DEFAULT_LOSS_OUTPUT_DIR)
        with self.assertRaises(runner.TrialFailed):
            runner.assert_output_dir_allowed("loss_goodput", runner.DEFAULT_WINDOW_OUTPUT_DIR)
        with self.assertRaises(runner.TrialFailed):
            runner.assert_output_dir_allowed("rto_sensitivity", runner.DEFAULT_WINDOW_OUTPUT_DIR)
        runner.assert_output_dir_allowed("primary_reordering", runner.DEFAULT_PRIMARY_OUTPUT_DIR)

    def test_study_filters_stay_on_their_own_study(self) -> None:
        window_args = runner.parse_args(["--study", "window", "--window", "8", "--seed", "101"])
        self.assertEqual(window_args.window, [8])
        loss_args = runner.parse_args(["--study", "loss", "--loss", "0.25"])
        self.assertEqual(loss_args.loss, [0.25])
        self.assertEqual(
            runner._reject_foreign_filters(runner.parse_args(["--study", "window", "--reorder", "0.1"])),
            "--reorder is only valid with --study primary",
        )
        self.assertEqual(
            runner._reject_foreign_filters(runner.parse_args(["--study", "loss", "--multiplier", "0.5"])),
            "--multiplier is only valid with --study rto",
        )
        self.assertIsNone(runner._reject_foreign_filters(runner.parse_args(["--study", "primary"])))

    def test_e4_does_not_synthesize_measurements(self) -> None:
        source = Path(runner.__file__).read_text(encoding="utf-8")
        self.assertIn("rto_sensitivity_study", source)
        self.assertNotIn("import uuid", source)
        self.assertNotIn("from uuid", source)
        self.assertNotIn("datetime.now", source)
        self.assertNotIn("time.time", source)
        self.assertNotIn("numpy.random", source)
        self.assertNotIn("random.uniform", source)
        self.assertNotIn("random.gauss", source)
        self.assertNotIn("fake_premature", source)

    def _temp_dir(self):
        import tempfile

        return tempfile.TemporaryDirectory(prefix="rdt-runner-test-")


if __name__ == "__main__":
    unittest.main()
