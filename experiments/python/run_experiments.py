#!/usr/bin/env python3
"""Run virtual-time experiment matrices via the Rust driver.

This module does not estimate retransmissions, delay, goodput, or RTT.
Every stored measurement is the JSON object written by `run-experiment`.

Studies (explicit --study required; bare invocation does not run anything):

* primary — E1 reordering matrix (3 × 6 × 5 = 90). Frozen data lives under results/raw/.
* rto     — E4 RTO-multiplier matrix (3 × 3 × 5 = 45). Writes only under results/rto_sensitivity/.
* window  — goodput versus window size (65 trials). Writes only under results/window_goodput/.
* loss    — goodput versus loss rate (90 trials). Writes only under results/loss_goodput/.

The window and loss studies must not regenerate results/raw/ or results/rto_sensitivity/.
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Mapping, Optional, Sequence

REPO_ROOT = Path(__file__).resolve().parents[2]


def resolve_default_binary() -> Path:
    """Prefer CARGO_TARGET_DIR/release when set (e.g. sandbox builds)."""

    cargo_target = os.environ.get("CARGO_TARGET_DIR")
    if cargo_target:
        candidate = Path(cargo_target) / "release" / "reliable_udp"
        if candidate.is_file():
            return candidate
    return REPO_ROOT / "target" / "release" / "reliable_udp"


DEFAULT_BINARY = resolve_default_binary()
DEFAULT_INPUT = REPO_ROOT / "fixtures" / "transfer_medium.bin"
DEFAULT_PRIMARY_OUTPUT_DIR = REPO_ROOT / "results" / "raw"
DEFAULT_RTO_OUTPUT_DIR = REPO_ROOT / "results" / "rto_sensitivity"
DEFAULT_WINDOW_OUTPUT_DIR = REPO_ROOT / "results" / "window_goodput"
DEFAULT_LOSS_OUTPUT_DIR = REPO_ROOT / "results" / "loss_goodput"

# Directory name under results/ that each study owns. Other studies must not write there.
_STUDY_RESULTS_DIR = {
    "primary_reordering": "raw",
    "rto_sensitivity": "rto_sensitivity",
    "window_goodput": "window_goodput",
    "loss_goodput": "loss_goodput",
}

# Column order of ExperimentRecord::csv_header() in src/metrics/collector.rs.
RUST_CSV_HEADER = (
    "experiment_id,protocol,window_size,chunk_size,segment_count,seed,reverse_seed,trial_id,"
    "configured_loss_rate,configured_reorder_rate,configured_duplicate_rate,configured_corrupt_rate,"
    "base_delay_ms,jitter_ms,reorder_extra_ms,"
    "reverse_loss_rate,reverse_reorder_rate,reverse_duplicate_rate,reverse_corrupt_rate,"
    "reverse_base_delay_ms,reverse_jitter_ms,reverse_reorder_extra_ms,"
    "rto_multiplier,initial_rto_ms,min_rto_ms,max_rto_ms,alpha,beta,k,"
    "source_bytes,delivered_unique_bytes,transfer_status,sha256_match,source_sha256,delivered_sha256,"
    "duration_secs,total_rtt_samples,karn_rejected_count,min_rtt_ms,max_rtt_ms,mean_rtt_ms,"
    "final_srtt_ms,final_rttvar_ms,final_rto_ms,timeout_count,backoff_count,"
    "data_sent,ack_sent,data_received,ack_received,data_retransmissions,timeout_retransmissions,"
    "premature_retransmissions,duplicate_data_received,duplicate_acks_received,"
    "offered_datagrams,dropped_datagrams,duplicated_datagrams,corrupted_datagrams,"
    "reordered_datagrams,scheduled_deliveries,"
    "reverse_offered_datagrams,reverse_dropped_datagrams,reverse_duplicated_datagrams,"
    "reverse_corrupted_datagrams,reverse_reordered_datagrams,reverse_scheduled_deliveries,"
    "goodput_bytes_sec,retransmission_ratio"
)

# (csv column, path inside the Rust JSON object). One entry per header column.
FLATTEN_PATHS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("experiment_id", ("meta", "experiment_id")),
    ("protocol", ("meta", "protocol")),
    ("window_size", ("meta", "window_size")),
    ("chunk_size", ("meta", "chunk_size")),
    ("segment_count", ("meta", "segment_count")),
    ("seed", ("meta", "seed")),
    ("reverse_seed", ("meta", "reverse_seed")),
    ("trial_id", ("meta", "trial_id")),
    ("configured_loss_rate", ("forward_config", "configured_loss_rate")),
    ("configured_reorder_rate", ("forward_config", "configured_reorder_rate")),
    ("configured_duplicate_rate", ("forward_config", "configured_duplicate_rate")),
    ("configured_corrupt_rate", ("forward_config", "configured_corrupt_rate")),
    ("base_delay_ms", ("forward_config", "base_delay_ms")),
    ("jitter_ms", ("forward_config", "jitter_ms")),
    ("reorder_extra_ms", ("forward_config", "reorder_extra_ms")),
    ("reverse_loss_rate", ("reverse_config", "configured_loss_rate")),
    ("reverse_reorder_rate", ("reverse_config", "configured_reorder_rate")),
    ("reverse_duplicate_rate", ("reverse_config", "configured_duplicate_rate")),
    ("reverse_corrupt_rate", ("reverse_config", "configured_corrupt_rate")),
    ("reverse_base_delay_ms", ("reverse_config", "base_delay_ms")),
    ("reverse_jitter_ms", ("reverse_config", "jitter_ms")),
    ("reverse_reorder_extra_ms", ("reverse_config", "reorder_extra_ms")),
    ("rto_multiplier", ("rto_config", "rto_multiplier")),
    ("initial_rto_ms", ("rto_config", "initial_rto_ms")),
    ("min_rto_ms", ("rto_config", "min_rto_ms")),
    ("max_rto_ms", ("rto_config", "max_rto_ms")),
    ("alpha", ("rto_config", "alpha")),
    ("beta", ("rto_config", "beta")),
    ("k", ("rto_config", "k")),
    ("source_bytes", ("app", "source_bytes")),
    ("delivered_unique_bytes", ("app", "delivered_unique_bytes")),
    ("transfer_status", ("app", "transfer_status")),
    ("sha256_match", ("app", "sha256_match")),
    ("source_sha256", ("app", "source_sha256")),
    ("delivered_sha256", ("app", "delivered_sha256")),
    ("duration_secs", ("timing", "duration_secs")),
    ("total_rtt_samples", ("timing", "total_rtt_samples")),
    ("karn_rejected_count", ("timing", "karn_rejected_count")),
    ("min_rtt_ms", ("timing", "min_rtt_ms")),
    ("max_rtt_ms", ("timing", "max_rtt_ms")),
    ("mean_rtt_ms", ("timing", "mean_rtt_ms")),
    ("final_srtt_ms", ("timing", "final_srtt_ms")),
    ("final_rttvar_ms", ("timing", "final_rttvar_ms")),
    ("final_rto_ms", ("timing", "final_rto_ms")),
    ("timeout_count", ("timing", "timeout_count")),
    ("backoff_count", ("timing", "backoff_count")),
    ("data_sent", ("protocol", "data_sent")),
    ("ack_sent", ("protocol", "ack_sent")),
    ("data_received", ("protocol", "data_received")),
    ("ack_received", ("protocol", "ack_received")),
    ("data_retransmissions", ("protocol", "data_retransmissions")),
    ("timeout_retransmissions", ("protocol", "timeout_retransmissions")),
    ("premature_retransmissions", ("protocol", "premature_retransmissions")),
    ("duplicate_data_received", ("protocol", "duplicate_data_received")),
    ("duplicate_acks_received", ("protocol", "duplicate_acks_received")),
    ("offered_datagrams", ("channel", "offered_datagrams")),
    ("dropped_datagrams", ("channel", "dropped_datagrams")),
    ("duplicated_datagrams", ("channel", "duplicated_datagrams")),
    ("corrupted_datagrams", ("channel", "corrupted_datagrams")),
    ("reordered_datagrams", ("channel", "reordered_datagrams")),
    ("scheduled_deliveries", ("channel", "scheduled_deliveries")),
    ("reverse_offered_datagrams", ("reverse_channel", "offered_datagrams")),
    ("reverse_dropped_datagrams", ("reverse_channel", "dropped_datagrams")),
    ("reverse_duplicated_datagrams", ("reverse_channel", "duplicated_datagrams")),
    ("reverse_corrupted_datagrams", ("reverse_channel", "corrupted_datagrams")),
    ("reverse_reordered_datagrams", ("reverse_channel", "reordered_datagrams")),
    ("reverse_scheduled_deliveries", ("reverse_channel", "scheduled_deliveries")),
    ("goodput_bytes_sec", ("goodput_bytes_sec",)),
    ("retransmission_ratio", ("retransmission_ratio",)),
)

CSV_COLUMNS = tuple(name for name, _path in FLATTEN_PATHS)

REQUIRED_SECTIONS = (
    "meta",
    "forward_config",
    "reverse_config",
    "rto_config",
    "app",
    "timing",
    "protocol",
    "channel",
    "reverse_channel",
)
REQUIRED_APP_FIELDS = (
    "source_bytes",
    "delivered_unique_bytes",
    "transfer_status",
    "sha256_match",
    "source_sha256",
    "delivered_sha256",
)
REQUIRED_META_FIELDS = ("experiment_id", "protocol", "seed", "trial_id")
DERIVED_FIELDS = ("goodput_bytes_sec", "retransmission_ratio")


@dataclass(frozen=True)
class StudyConfig:
    """Configured factors for a study. These are inputs, not measurements."""

    name: str
    protocols: tuple[str, ...]
    reorder_rates: tuple[float, ...]
    rto_multipliers: tuple[float, ...]
    loss_rate: float
    seeds: tuple[int, ...]
    window_size: int
    chunk_size: int
    duplicate_rate: float
    corrupt_rate: float
    base_delay_ms: int
    jitter_ms: int
    reorder_extra_ms: int
    min_rto_ms: int
    max_rto_ms: int
    initial_rto_ms: int
    input_file: Path
    # Empty means the scalar loss_rate / window_size above is the only value.
    # Primary and RTO leave these empty so their matrices stay unchanged.
    loss_rates: tuple[float, ...] = ()
    window_sizes: tuple[int, ...] = ()

    @property
    def rto_multiplier(self) -> float:
        """Primary studies use a single fixed multiplier; E4 varies the tuple."""

        return self.rto_multipliers[0]


@dataclass(frozen=True)
class TrialSpec:
    experiment_id: str
    protocol: str
    window: int
    chunk_size: int
    seed: int
    trial_id: int
    loss_rate: float
    reorder_rate: float
    duplicate_rate: float
    corrupt_rate: float
    base_delay_ms: int
    jitter_ms: int
    reorder_extra_ms: int
    rto_multiplier: float
    min_rto_ms: int
    max_rto_ms: int
    initial_rto_ms: int
    input_file: Path


class TrialFailed(RuntimeError):
    """A Rust trial exited non-zero, or its JSON was not an acceptable record."""


def primary_reordering_study(input_file: Path = DEFAULT_INPUT) -> StudyConfig:
    """E1 from the project methodology: vary reorder, hold loss at 0.

    Stop-and-Wait uses window 1. Go-Back-N and Selective Repeat use window 8.
    Five matched seeds are repeated at each reorder rate. Alpha, beta, and k
    are fixed inside the Rust estimator and are not CLI inputs.
    """

    return StudyConfig(
        name="primary_reordering",
        protocols=("StopAndWait", "GoBackN", "SelectiveRepeat"),
        reorder_rates=(0.0, 0.05, 0.10, 0.15, 0.20, 0.25),
        rto_multipliers=(1.0,),
        loss_rate=0.0,
        seeds=(101, 202, 303, 404, 505),
        window_size=8,
        chunk_size=1400,
        duplicate_rate=0.0,
        corrupt_rate=0.0,
        base_delay_ms=20,
        jitter_ms=0,
        reorder_extra_ms=50,
        min_rto_ms=200,
        max_rto_ms=60000,
        initial_rto_ms=1000,
        input_file=input_file,
    )


def rto_sensitivity_study(input_file: Path = DEFAULT_INPUT) -> StudyConfig:
    """E4 secondary study: vary RTO multiplier under fixed 5% loss, no reorder.

    min_rto is 10 ms for E4 only so the 0.5× arm is not silently clamped by the
    primary study's 200 ms floor. Alpha, beta, and k remain the Rust defaults.
    """

    return StudyConfig(
        name="rto_sensitivity",
        protocols=("StopAndWait", "GoBackN", "SelectiveRepeat"),
        reorder_rates=(0.0,),
        rto_multipliers=(0.5, 1.0, 3.0),
        loss_rate=0.05,
        seeds=(101, 202, 303, 404, 505),
        window_size=8,
        chunk_size=1400,
        duplicate_rate=0.0,
        corrupt_rate=0.0,
        base_delay_ms=20,
        jitter_ms=0,
        reorder_extra_ms=50,
        min_rto_ms=10,
        max_rto_ms=60000,
        initial_rto_ms=1000,
        input_file=input_file,
    )


def window_goodput_study(input_file: Path = DEFAULT_INPUT) -> StudyConfig:
    """Goodput versus window size. Loss and reorder stay at 0.

    Go-Back-N and Selective Repeat use windows 1, 2, 4, 8, 16, and 32.
    Stop-and-Wait is emitted once, at window 1. Same seeds, file, chunk,
    delay, and RTO bounds as the primary study (floor 200 ms, not the E4 floor).
    """

    return StudyConfig(
        name="window_goodput",
        protocols=("StopAndWait", "GoBackN", "SelectiveRepeat"),
        reorder_rates=(0.0,),
        rto_multipliers=(1.0,),
        loss_rate=0.0,
        seeds=(101, 202, 303, 404, 505),
        window_size=8,
        chunk_size=1400,
        duplicate_rate=0.0,
        corrupt_rate=0.0,
        base_delay_ms=20,
        jitter_ms=0,
        reorder_extra_ms=50,
        min_rto_ms=200,
        max_rto_ms=60000,
        initial_rto_ms=1000,
        input_file=input_file,
        window_sizes=(1, 2, 4, 8, 16, 32),
    )


def loss_goodput_study(input_file: Path = DEFAULT_INPUT) -> StudyConfig:
    """Goodput versus loss rate. Reorder stays at 0. Window stays at 8.

    Loss rates mirror the primary reorder grid. RTO bounds match the primary
    study, including the 200 ms floor. Stop-and-Wait still runs at window 1.
    """

    return StudyConfig(
        name="loss_goodput",
        protocols=("StopAndWait", "GoBackN", "SelectiveRepeat"),
        reorder_rates=(0.0,),
        rto_multipliers=(1.0,),
        loss_rate=0.0,
        seeds=(101, 202, 303, 404, 505),
        window_size=8,
        chunk_size=1400,
        duplicate_rate=0.0,
        corrupt_rate=0.0,
        base_delay_ms=20,
        jitter_ms=0,
        reorder_extra_ms=50,
        min_rto_ms=200,
        max_rto_ms=60000,
        initial_rto_ms=1000,
        input_file=input_file,
        loss_rates=(0.0, 0.05, 0.10, 0.15, 0.20, 0.25),
    )


def format_rate(rate: float) -> str:
    return f"{rate:.2f}"


def window_for(protocol: str, study: StudyConfig) -> int:
    if protocol == "StopAndWait":
        return 1
    return study.window_size


def primary_experiment_id(protocol: str, reorder_rate: float, seed: int, trial_id: int) -> str:
    return (
        f"primary_reordering_{protocol}_r{format_rate(reorder_rate)}"
        f"_s{seed}_t{trial_id}"
    )


def rto_experiment_id(protocol: str, multiplier: float, seed: int, trial_id: int) -> str:
    return (
        f"rto_sensitivity_{protocol}_m{format_rate(multiplier)}"
        f"_s{seed}_t{trial_id}"
    )


def window_experiment_id(protocol: str, window: int, seed: int, trial_id: int) -> str:
    return f"window_goodput_{protocol}_w{window}_s{seed}_t{trial_id}"


def loss_experiment_id(protocol: str, loss_rate: float, seed: int, trial_id: int) -> str:
    return f"loss_goodput_{protocol}_l{format_rate(loss_rate)}_s{seed}_t{trial_id}"


# Backwards-compatible alias used by existing primary tests.
experiment_id = primary_experiment_id


def _selected(value: float, chosen: Optional[Sequence[float]]) -> bool:
    if chosen is None:
        return True
    return any(format_rate(value) == format_rate(item) for item in chosen)


def _make_trial(
    study: StudyConfig,
    *,
    experiment_id_value: str,
    protocol: str,
    seed: int,
    trial_id: int,
    reorder_rate: float,
    rto_multiplier: float,
    window: Optional[int] = None,
    loss_rate: Optional[float] = None,
) -> TrialSpec:
    # Stop-and-Wait is defined as one packet in flight. A swept window must not
    # change that, even if a caller passes another value.
    if protocol == "StopAndWait":
        chosen_window = 1
    elif window is not None:
        chosen_window = window
    else:
        chosen_window = window_for(protocol, study)
    chosen_loss = study.loss_rate if loss_rate is None else loss_rate
    return TrialSpec(
        experiment_id=experiment_id_value,
        protocol=protocol,
        window=chosen_window,
        chunk_size=study.chunk_size,
        seed=seed,
        trial_id=trial_id,
        loss_rate=chosen_loss,
        reorder_rate=reorder_rate,
        duplicate_rate=study.duplicate_rate,
        corrupt_rate=study.corrupt_rate,
        base_delay_ms=study.base_delay_ms,
        jitter_ms=study.jitter_ms,
        reorder_extra_ms=study.reorder_extra_ms,
        rto_multiplier=rto_multiplier,
        min_rto_ms=study.min_rto_ms,
        max_rto_ms=study.max_rto_ms,
        initial_rto_ms=study.initial_rto_ms,
        input_file=study.input_file,
    )


def expand_primary_trials(
    study: StudyConfig,
    protocols: Optional[Sequence[str]] = None,
    reorder_rates: Optional[Sequence[float]] = None,
    seeds: Optional[Sequence[int]] = None,
    max_trials: Optional[int] = None,
) -> list[TrialSpec]:
    """Expand the primary study in a fixed order. trial_id follows the full seed list."""

    if study.name != "primary_reordering":
        raise TrialFailed(f"expand_primary_trials requires primary_reordering, got {study.name!r}")
    protocol_filter = set(protocols) if protocols is not None else None
    seed_filter = set(seeds) if seeds is not None else None
    trials: list[TrialSpec] = []
    for protocol in study.protocols:
        if protocol_filter is not None and protocol not in protocol_filter:
            continue
        for reorder_rate in study.reorder_rates:
            if not _selected(reorder_rate, reorder_rates):
                continue
            for index, seed in enumerate(study.seeds, start=1):
                if seed_filter is not None and seed not in seed_filter:
                    continue
                trials.append(
                    _make_trial(
                        study,
                        experiment_id_value=primary_experiment_id(protocol, reorder_rate, seed, index),
                        protocol=protocol,
                        seed=seed,
                        trial_id=index,
                        reorder_rate=reorder_rate,
                        rto_multiplier=study.rto_multiplier,
                    )
                )
    if max_trials is not None:
        return trials[:max_trials]
    return trials


def expand_rto_trials(
    study: StudyConfig,
    protocols: Optional[Sequence[str]] = None,
    multipliers: Optional[Sequence[float]] = None,
    seeds: Optional[Sequence[int]] = None,
    max_trials: Optional[int] = None,
) -> list[TrialSpec]:
    """Expand E4: protocol × RTO multiplier × seed. trial_id follows the full seed list."""

    if study.name != "rto_sensitivity":
        raise TrialFailed(f"expand_rto_trials requires rto_sensitivity, got {study.name!r}")
    protocol_filter = set(protocols) if protocols is not None else None
    seed_filter = set(seeds) if seeds is not None else None
    reorder_rate = study.reorder_rates[0]
    trials: list[TrialSpec] = []
    for protocol in study.protocols:
        if protocol_filter is not None and protocol not in protocol_filter:
            continue
        for multiplier in study.rto_multipliers:
            if not _selected(multiplier, multipliers):
                continue
            for index, seed in enumerate(study.seeds, start=1):
                if seed_filter is not None and seed not in seed_filter:
                    continue
                trials.append(
                    _make_trial(
                        study,
                        experiment_id_value=rto_experiment_id(protocol, multiplier, seed, index),
                        protocol=protocol,
                        seed=seed,
                        trial_id=index,
                        reorder_rate=reorder_rate,
                        rto_multiplier=multiplier,
                    )
                )
    if max_trials is not None:
        return trials[:max_trials]
    return trials


def expand_window_trials(
    study: StudyConfig,
    protocols: Optional[Sequence[str]] = None,
    windows: Optional[Sequence[int]] = None,
    seeds: Optional[Sequence[int]] = None,
    max_trials: Optional[int] = None,
) -> list[TrialSpec]:
    """Expand the window study. Stop-and-Wait is only emitted at window 1."""

    if study.name != "window_goodput":
        raise TrialFailed(f"expand_window_trials requires window_goodput, got {study.name!r}")
    if not study.window_sizes:
        raise TrialFailed("window_goodput study has no window_sizes")
    protocol_filter = set(protocols) if protocols is not None else None
    seed_filter = set(seeds) if seeds is not None else None
    window_filter = set(windows) if windows is not None else None
    reorder_rate = study.reorder_rates[0]
    trials: list[TrialSpec] = []
    for protocol in study.protocols:
        if protocol_filter is not None and protocol not in protocol_filter:
            continue
        protocol_windows = (1,) if protocol == "StopAndWait" else study.window_sizes
        for window in protocol_windows:
            if window_filter is not None and window not in window_filter:
                continue
            for index, seed in enumerate(study.seeds, start=1):
                if seed_filter is not None and seed not in seed_filter:
                    continue
                trials.append(
                    _make_trial(
                        study,
                        experiment_id_value=window_experiment_id(protocol, window, seed, index),
                        protocol=protocol,
                        seed=seed,
                        trial_id=index,
                        reorder_rate=reorder_rate,
                        rto_multiplier=study.rto_multiplier,
                        window=window,
                    )
                )
    if max_trials is not None:
        return trials[:max_trials]
    return trials


def expand_loss_trials(
    study: StudyConfig,
    protocols: Optional[Sequence[str]] = None,
    loss_rates: Optional[Sequence[float]] = None,
    seeds: Optional[Sequence[int]] = None,
    max_trials: Optional[int] = None,
) -> list[TrialSpec]:
    """Expand the loss study: protocol × loss rate × seed."""

    if study.name != "loss_goodput":
        raise TrialFailed(f"expand_loss_trials requires loss_goodput, got {study.name!r}")
    if not study.loss_rates:
        raise TrialFailed("loss_goodput study has no loss_rates")
    protocol_filter = set(protocols) if protocols is not None else None
    seed_filter = set(seeds) if seeds is not None else None
    reorder_rate = study.reorder_rates[0]
    trials: list[TrialSpec] = []
    for protocol in study.protocols:
        if protocol_filter is not None and protocol not in protocol_filter:
            continue
        for loss_rate in study.loss_rates:
            if not _selected(loss_rate, loss_rates):
                continue
            for index, seed in enumerate(study.seeds, start=1):
                if seed_filter is not None and seed not in seed_filter:
                    continue
                trials.append(
                    _make_trial(
                        study,
                        experiment_id_value=loss_experiment_id(protocol, loss_rate, seed, index),
                        protocol=protocol,
                        seed=seed,
                        trial_id=index,
                        reorder_rate=reorder_rate,
                        rto_multiplier=study.rto_multiplier,
                        loss_rate=loss_rate,
                    )
                )
    if max_trials is not None:
        return trials[:max_trials]
    return trials


def expand_trials(
    study: StudyConfig,
    protocols: Optional[Sequence[str]] = None,
    reorder_rates: Optional[Sequence[float]] = None,
    multipliers: Optional[Sequence[float]] = None,
    seeds: Optional[Sequence[int]] = None,
    max_trials: Optional[int] = None,
    windows: Optional[Sequence[int]] = None,
    loss_rates: Optional[Sequence[float]] = None,
) -> list[TrialSpec]:
    """Expand the given study. Dispatches on study.name."""

    if study.name == "rto_sensitivity":
        return expand_rto_trials(
            study,
            protocols=protocols,
            multipliers=multipliers,
            seeds=seeds,
            max_trials=max_trials,
        )
    if study.name == "window_goodput":
        return expand_window_trials(
            study,
            protocols=protocols,
            windows=windows,
            seeds=seeds,
            max_trials=max_trials,
        )
    if study.name == "loss_goodput":
        return expand_loss_trials(
            study,
            protocols=protocols,
            loss_rates=loss_rates,
            seeds=seeds,
            max_trials=max_trials,
        )
    return expand_primary_trials(
        study,
        protocols=protocols,
        reorder_rates=reorder_rates,
        seeds=seeds,
        max_trials=max_trials,
    )


def build_command(binary: Path, trial: TrialSpec, json_out: Path) -> list[str]:
    """Argument vector for one `run-experiment` process. Not a shell string."""

    return [
        str(binary),
        "run-experiment",
        "--file",
        str(trial.input_file),
        "--protocol",
        trial.protocol,
        "--window",
        str(trial.window),
        "--chunk-size",
        str(trial.chunk_size),
        "--seed",
        str(trial.seed),
        "--trial-id",
        str(trial.trial_id),
        "--experiment-id",
        trial.experiment_id,
        "--loss",
        format_rate(trial.loss_rate),
        "--reorder",
        format_rate(trial.reorder_rate),
        "--duplicate",
        format_rate(trial.duplicate_rate),
        "--corrupt",
        format_rate(trial.corrupt_rate),
        "--base-delay-ms",
        str(trial.base_delay_ms),
        "--jitter-ms",
        str(trial.jitter_ms),
        "--reorder-extra-ms",
        str(trial.reorder_extra_ms),
        "--rto-multiplier",
        f"{trial.rto_multiplier:.6f}",
        "--min-rto-ms",
        str(trial.min_rto_ms),
        "--max-rto-ms",
        str(trial.max_rto_ms),
        "--initial-rto-ms",
        str(trial.initial_rto_ms),
        "--json-out",
        str(json_out),
    ]


def _lookup(record: Mapping[str, Any], path: tuple[str, ...]) -> Any:
    current: Any = record
    for key in path:
        if not isinstance(current, Mapping) or key not in current:
            raise TrialFailed(f"record is missing {'.'.join(path)}")
        current = current[key]
    return current


def _floats_match(left: Any, right: float, *, places: int = 6) -> bool:
    return format(float(left), f".{places}f") == format(right, f".{places}f")


def validate_record(record: Mapping[str, Any], trial: TrialSpec) -> None:
    if not isinstance(record, Mapping):
        raise TrialFailed(f"{trial.experiment_id}: JSON root is not an object")
    for section in REQUIRED_SECTIONS:
        if section not in record or not isinstance(record[section], Mapping):
            raise TrialFailed(f"{trial.experiment_id}: missing section {section}")
    for field in DERIVED_FIELDS:
        if field not in record:
            raise TrialFailed(f"{trial.experiment_id}: missing {field}")
    meta = record["meta"]
    for field in REQUIRED_META_FIELDS:
        if field not in meta:
            raise TrialFailed(f"{trial.experiment_id}: meta.{field} is missing")
    app = record["app"]
    for field in REQUIRED_APP_FIELDS:
        if field not in app:
            raise TrialFailed(f"{trial.experiment_id}: app.{field} is missing")
    if meta["experiment_id"] != trial.experiment_id:
        raise TrialFailed(
            f"{trial.experiment_id}: Rust returned experiment_id {meta['experiment_id']!r}"
        )
    if meta["protocol"] != trial.protocol:
        raise TrialFailed(f"{trial.experiment_id}: Rust returned protocol {meta['protocol']!r}")
    if int(meta["seed"]) != trial.seed or int(meta["trial_id"]) != trial.trial_id:
        raise TrialFailed(f"{trial.experiment_id}: seed or trial_id does not match the request")
    if int(meta.get("window_size", -1)) != trial.window:
        raise TrialFailed(
            f"{trial.experiment_id}: Rust returned window_size {meta.get('window_size')!r}"
        )
    if int(meta.get("chunk_size", -1)) != trial.chunk_size:
        raise TrialFailed(
            f"{trial.experiment_id}: Rust returned chunk_size {meta.get('chunk_size')!r}"
        )

    forward = record["forward_config"]
    if not _floats_match(forward["configured_loss_rate"], trial.loss_rate):
        raise TrialFailed(
            f"{trial.experiment_id}: loss mismatch "
            f"(got {forward['configured_loss_rate']}, want {trial.loss_rate})"
        )
    if not _floats_match(forward["configured_reorder_rate"], trial.reorder_rate):
        raise TrialFailed(
            f"{trial.experiment_id}: reorder mismatch "
            f"(got {forward['configured_reorder_rate']}, want {trial.reorder_rate})"
        )
    if not _floats_match(forward["configured_duplicate_rate"], trial.duplicate_rate):
        raise TrialFailed(f"{trial.experiment_id}: duplicate rate mismatch")
    if not _floats_match(forward["configured_corrupt_rate"], trial.corrupt_rate):
        raise TrialFailed(f"{trial.experiment_id}: corrupt rate mismatch")
    if int(forward["base_delay_ms"]) != trial.base_delay_ms:
        raise TrialFailed(f"{trial.experiment_id}: base_delay_ms mismatch")
    if int(forward["jitter_ms"]) != trial.jitter_ms:
        raise TrialFailed(f"{trial.experiment_id}: jitter_ms mismatch")
    if int(forward["reorder_extra_ms"]) != trial.reorder_extra_ms:
        raise TrialFailed(f"{trial.experiment_id}: reorder_extra_ms mismatch")

    rto = record["rto_config"]
    if not _floats_match(rto["rto_multiplier"], trial.rto_multiplier):
        raise TrialFailed(
            f"{trial.experiment_id}: rto_multiplier mismatch "
            f"(got {rto['rto_multiplier']}, want {trial.rto_multiplier})"
        )
    if float(rto["min_rto_ms"]) != float(trial.min_rto_ms):
        raise TrialFailed(
            f"{trial.experiment_id}: min_rto_ms mismatch "
            f"(got {rto['min_rto_ms']}, want {trial.min_rto_ms})"
        )
    if float(rto["max_rto_ms"]) != float(trial.max_rto_ms):
        raise TrialFailed(
            f"{trial.experiment_id}: max_rto_ms mismatch "
            f"(got {rto['max_rto_ms']}, want {trial.max_rto_ms})"
        )
    if float(rto["initial_rto_ms"]) != float(trial.initial_rto_ms):
        raise TrialFailed(
            f"{trial.experiment_id}: initial_rto_ms mismatch "
            f"(got {rto['initial_rto_ms']}, want {trial.initial_rto_ms})"
        )

    if app["transfer_status"] != "SUCCESS" or app["sha256_match"] is not True:
        raise TrialFailed(
            f"{trial.experiment_id}: transfer_status={app['transfer_status']!r} "
            f"sha256_match={app['sha256_match']!r}"
        )


def csv_cell(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if value is None:
        raise TrialFailed("record contains a null measurement")
    return str(value)


def flatten_record(record: Mapping[str, Any]) -> dict[str, str]:
    return {name: csv_cell(_lookup(record, path)) for name, path in FLATTEN_PATHS}


def write_dataset(records: Sequence[Mapping[str, Any]], output_dir: Path) -> tuple[Path, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    json_path = output_dir / "experiments_raw.json"
    csv_path = output_dir / "experiments_raw.csv"
    json_path.write_text(json.dumps(list(records), indent=2) + "\n", encoding="utf-8")
    with csv_path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(CSV_COLUMNS), quoting=csv.QUOTE_MINIMAL)
        writer.writeheader()
        for record in records:
            writer.writerow(flatten_record(record))
    return json_path, csv_path


def execute_command(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, capture_output=True, text=True, check=False)


def run_one_trial(
    binary: Path,
    trial: TrialSpec,
    trials_dir: Path,
    execute: Callable[[Sequence[str]], subprocess.CompletedProcess[str]] = execute_command,
) -> dict[str, Any]:
    trials_dir.mkdir(parents=True, exist_ok=True)
    json_out = trials_dir / f"{trial.experiment_id}.json"
    command = build_command(binary, trial, json_out)
    completed = execute(command)
    if completed.returncode != 0:
        raise TrialFailed(
            f"trial {trial.experiment_id} failed with exit {completed.returncode}\n"
            f"command: {command}\n"
            f"stdout:\n{completed.stdout}\n"
            f"stderr:\n{completed.stderr}"
        )
    if not json_out.is_file():
        raise TrialFailed(
            f"trial {trial.experiment_id} exited 0 but did not write {json_out}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    record = json.loads(json_out.read_text(encoding="utf-8"))
    validate_record(record, trial)
    return record


def run_trials(
    trials: Sequence[TrialSpec],
    binary: Path,
    output_dir: Path,
    execute: Callable[[Sequence[str]], subprocess.CompletedProcess[str]] = execute_command,
) -> tuple[Path, Path]:
    if not trials:
        raise TrialFailed("no trials selected")
    records = [run_one_trial(binary, trial, output_dir / "trials", execute) for trial in trials]
    return write_dataset(records, output_dir)


def ensure_release_binary(binary: Path, build: bool) -> Path:
    if build or not binary.is_file():
        subprocess.run(
            ["cargo", "build", "--release"],
            cwd=REPO_ROOT,
            check=True,
        )
    if binary.is_file():
        return binary
    # After a sandbox build, the artifact may live under CARGO_TARGET_DIR.
    fallback = resolve_default_binary()
    if fallback.is_file() and fallback != binary:
        return fallback
    raise TrialFailed(f"release binary not found: {binary}")


def _under_results_child(output_dir: Path, child_name: str) -> bool:
    """True when output_dir is results/<child_name> or a directory inside it."""

    root = (REPO_ROOT / "results" / child_name).resolve()
    resolved = output_dir.resolve()
    if resolved == root or root in resolved.parents:
        return True
    parts = output_dir.parts
    for index in range(len(parts) - 1):
        if parts[index] == "results" and parts[index + 1] == child_name:
            return True
    return False


def assert_output_dir_allowed(study_name: str, output_dir: Path) -> None:
    """Non-primary studies must not write into another study's results directory.

    Primary is unchanged: this guard returns immediately for primary_reordering.
    E4, the window study, and the loss study each refuse results/raw/ and each
    other's default directories.
    """

    if study_name == "primary_reordering":
        return
    if study_name not in _STUDY_RESULTS_DIR:
        raise TrialFailed(f"unknown study for output guard: {study_name}")
    for name, child in _STUDY_RESULTS_DIR.items():
        if name == study_name:
            continue
        if _under_results_child(output_dir, child):
            raise TrialFailed(
                f"{study_name} must not write under results/{child}/ (got {output_dir})"
            )


def parse_args(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run a virtual-time study through the Rust release binary. "
            "--study is required so a bare invocation cannot overwrite a frozen dataset."
        )
    )
    parser.add_argument(
        "--study",
        required=True,
        choices=["primary", "rto", "window", "loss"],
        help=(
            "primary = E1 reordering (results/raw); "
            "rto = E4 (results/rto_sensitivity); "
            "window = goodput vs window (results/window_goodput); "
            "loss = goodput vs loss (results/loss_goodput)."
        ),
    )
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--build", action="store_true", help="Run cargo build --release first.")
    parser.add_argument("--input-file", type=Path, default=DEFAULT_INPUT)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help="Defaults to that study's own results directory. Non-primary studies cannot use results/raw/.",
    )
    parser.add_argument("--protocol", action="append", choices=["StopAndWait", "GoBackN", "SelectiveRepeat"])
    parser.add_argument("--reorder", type=float, action="append", help="Primary study filter.")
    parser.add_argument("--multiplier", type=float, action="append", help="E4 RTO multiplier filter.")
    parser.add_argument("--window", type=int, action="append", help="Window-study filter.")
    parser.add_argument("--loss", type=float, action="append", help="Loss-study filter.")
    parser.add_argument("--seed", type=int, action="append")
    parser.add_argument("--max-trials", type=int, default=None)
    return parser.parse_args(argv)


def _reject_foreign_filters(args: argparse.Namespace) -> Optional[str]:
    """Return an error string when a filter belongs to a different study."""

    allowed = {
        "primary": {"reorder"},
        "rto": {"multiplier"},
        "window": {"window"},
        "loss": {"loss"},
    }[args.study]
    checks = (
        ("reorder", args.reorder, "--reorder is only valid with --study primary"),
        ("multiplier", args.multiplier, "--multiplier is only valid with --study rto"),
        ("window", args.window, "--window is only valid with --study window"),
        ("loss", args.loss, "--loss is only valid with --study loss"),
    )
    for name, value, message in checks:
        if value is not None and name not in allowed:
            return message
    return None


def _select_study(args: argparse.Namespace) -> tuple[StudyConfig, Path, list[TrialSpec]]:
    if args.study == "primary":
        study = primary_reordering_study(args.input_file)
        output_dir = args.output_dir if args.output_dir is not None else DEFAULT_PRIMARY_OUTPUT_DIR
        trials = expand_trials(
            study,
            protocols=args.protocol,
            reorder_rates=args.reorder,
            seeds=args.seed,
            max_trials=args.max_trials,
        )
        return study, output_dir, trials
    if args.study == "rto":
        study = rto_sensitivity_study(args.input_file)
        output_dir = args.output_dir if args.output_dir is not None else DEFAULT_RTO_OUTPUT_DIR
        trials = expand_trials(
            study,
            protocols=args.protocol,
            multipliers=args.multiplier,
            seeds=args.seed,
            max_trials=args.max_trials,
        )
        return study, output_dir, trials
    if args.study == "window":
        study = window_goodput_study(args.input_file)
        output_dir = args.output_dir if args.output_dir is not None else DEFAULT_WINDOW_OUTPUT_DIR
        trials = expand_trials(
            study,
            protocols=args.protocol,
            windows=args.window,
            seeds=args.seed,
            max_trials=args.max_trials,
        )
        return study, output_dir, trials
    study = loss_goodput_study(args.input_file)
    output_dir = args.output_dir if args.output_dir is not None else DEFAULT_LOSS_OUTPUT_DIR
    trials = expand_trials(
        study,
        protocols=args.protocol,
        loss_rates=args.loss,
        seeds=args.seed,
        max_trials=args.max_trials,
    )
    return study, output_dir, trials


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = parse_args(argv)
    rejected = _reject_foreign_filters(args)
    if rejected is not None:
        print(rejected, file=sys.stderr)
        return 1
    study, output_dir, trials = _select_study(args)
    if args.study != "primary":
        try:
            assert_output_dir_allowed(study.name, output_dir)
        except TrialFailed as exc:
            print(str(exc), file=sys.stderr)
            return 1

    if not study.input_file.is_file():
        print(f"input file not found: {study.input_file}", file=sys.stderr)
        return 1
    try:
        binary = ensure_release_binary(args.binary, args.build)
        json_path, csv_path = run_trials(trials, binary, output_dir)
    except (TrialFailed, subprocess.CalledProcessError, OSError, json.JSONDecodeError) as exc:
        print(str(exc), file=sys.stderr)
        return 1
    print(f"study: {study.name}")
    print(f"trials: {len(trials)}")
    print(f"json: {json_path}")
    print(f"csv: {csv_path}")
    return 0


# Alias kept for callers / docs that still use the old name.
DEFAULT_OUTPUT_DIR = DEFAULT_PRIMARY_OUTPUT_DIR


if __name__ == "__main__":
    raise SystemExit(main())
