#!/usr/bin/env python3
"""Analyse the window-size and loss-rate goodput studies.

Reads only that study's experiments_raw.json, plus the frozen primary
dataset for the identical-configuration regression check. Does not write
under results/raw/ or results/rto_sensitivity/. Does not invent measurements.

Means and 95% Student-t intervals use the same helpers as plot_results.py.
Failed or hash-mismatched trials are listed and left out of the means.
"""

from __future__ import annotations

import argparse
import csv
import json
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any, Mapping, Optional, Sequence

sys.path.insert(0, str(Path(__file__).resolve().parent))

import plot_results as stats
import run_experiments as runner

REPO = Path(__file__).resolve().parents[2]
PRIMARY_JSON = REPO / "results" / "raw" / "experiments_raw.json"

PROTOCOLS = ("StopAndWait", "GoBackN", "SelectiveRepeat")
PROTOCOL_COLOR = {
    "StopAndWait": "#c0392b",
    "GoBackN": "#d68910",
    "SelectiveRepeat": "#1e8449",
}
METRICS: tuple[tuple[str, tuple[str, ...], str, str], ...] = (
    ("goodput_bytes_sec", ("goodput_bytes_sec",), "bytes/s", "Goodput"),
    ("data_retransmissions", ("protocol", "data_retransmissions"), "packets", "Data retransmissions"),
    ("duration_secs", ("timing", "duration_secs"), "s", "Transfer duration"),
)
PAIRED_METRICS = ("goodput_bytes_sec", "data_retransmissions", "duration_secs")
REGRESSION_FIELDS: tuple[tuple[str, ...], ...] = (
    ("app", "transfer_status"),
    ("app", "sha256_match"),
    ("app", "delivered_sha256"),
    ("protocol", "data_retransmissions"),
    ("protocol", "premature_retransmissions"),
    ("timing", "timeout_count"),
    ("timing", "duration_secs"),
    ("goodput_bytes_sec",),
)


def study_for(kind: str) -> runner.StudyConfig:
    if kind == "window":
        return runner.window_goodput_study()
    if kind == "loss":
        return runner.loss_goodput_study()
    raise SystemExit(f"unknown study {kind!r}")


def output_dir_for(kind: str) -> Path:
    if kind == "window":
        return runner.DEFAULT_WINDOW_OUTPUT_DIR
    if kind == "loss":
        return runner.DEFAULT_LOSS_OUTPUT_DIR
    raise SystemExit(f"unknown study {kind!r}")


def factor_value(kind: str, record: Mapping[str, Any]) -> float:
    if kind == "window":
        return float(int(record["meta"]["window_size"]))
    return float(record["forward_config"]["configured_loss_rate"])


def factor_header(kind: str) -> str:
    return "window_size" if kind == "window" else "loss_rate"


def factor_axis_label(kind: str) -> str:
    if kind == "window":
        return "Window size (packets)"
    return "Configured loss rate"


def format_factor(kind: str, value: float) -> str:
    if kind == "window":
        return str(int(value))
    return runner.format_rate(value)


def is_success(record: Mapping[str, Any]) -> bool:
    app = record["app"]
    return app["transfer_status"] == "SUCCESS" and app["sha256_match"] is True


def _metric(record: Mapping[str, Any], path: tuple[str, ...]) -> Any:
    current: Any = record
    for key in path:
        current = current[key]
    return current


def load_records(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        raise SystemExit(f"dataset not found: {path}")
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, list):
        raise SystemExit(f"{path} is not a JSON list")
    return payload


def validate_matrix(kind: str, records: Sequence[Mapping[str, Any]]) -> list[Mapping[str, Any]]:
    """Check ids, counts, and recorded controls. Return the non-success records."""

    study = study_for(kind)
    expected = runner.expand_trials(study)
    if len(records) != len(expected):
        raise SystemExit(f"{kind}: expected {len(expected)} records, found {len(records)}")
    by_id = {str(record["meta"]["experiment_id"]): record for record in records}
    if len(by_id) != len(records):
        raise SystemExit(f"{kind}: duplicate experiment_id")
    failures: list[Mapping[str, Any]] = []
    for trial in expected:
        record = by_id.get(trial.experiment_id)
        if record is None:
            raise SystemExit(f"{kind}: missing {trial.experiment_id}")
        runner.validate_record(record, trial)
        if not is_success(record):
            failures.append(record)
    return failures


def aggregate_rows(kind: str, records: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    study = study_for(kind)
    successes = [record for record in records if is_success(record)]
    grouped: dict[tuple[str, str], list[Mapping[str, Any]]] = defaultdict(list)
    for record in successes:
        label = format_factor(kind, factor_value(kind, record))
        grouped[(str(record["meta"]["protocol"]), label)].append(record)

    if kind == "window":
        protocol_levels = {
            "StopAndWait": ("1",),
            "GoBackN": tuple(str(window) for window in study.window_sizes),
            "SelectiveRepeat": tuple(str(window) for window in study.window_sizes),
        }
    else:
        protocol_levels = {
            protocol: tuple(runner.format_rate(rate) for rate in study.loss_rates) for protocol in PROTOCOLS
        }

    header = factor_header(kind)
    rows: list[dict[str, Any]] = []
    for protocol in PROTOCOLS:
        for level in protocol_levels[protocol]:
            cell = grouped.get((protocol, level), [])
            for name, path, _unit, _title in METRICS:
                if not cell:
                    rows.append(_blank_row(header, protocol, level, name))
                    continue
                values = [float(_metric(record, path)) for record in cell]
                low, high = stats.confidence_interval_95(values)
                deviation = stats.sample_std(values)
                error = stats.standard_error(values)
                rows.append(
                    {
                        "protocol": protocol,
                        header: level,
                        "metric": name,
                        "n": len(values),
                        "mean": stats.mean(values),
                        "std": "" if deviation is None else deviation,
                        "sem": "" if error is None else error,
                        "ci95_low": "" if low is None else low,
                        "ci95_high": "" if high is None else high,
                    }
                )
    return rows


def _blank_row(header: str, protocol: str, level: str, metric: str) -> dict[str, Any]:
    return {
        "protocol": protocol,
        header: level,
        "metric": metric,
        "n": 0,
        "mean": "",
        "std": "",
        "sem": "",
        "ci95_low": "",
        "ci95_high": "",
    }


def paired_rows(kind: str, records: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    """Go-Back-N minus Selective Repeat, paired on seed, at each factor level."""

    study = study_for(kind)
    header = factor_header(kind)
    indexed: dict[tuple[str, str, int], Mapping[str, Any]] = {}
    for record in records:
        if not is_success(record):
            continue
        key = (
            str(record["meta"]["protocol"]),
            format_factor(kind, factor_value(kind, record)),
            int(record["meta"]["seed"]),
        )
        indexed[key] = record
    levels = (
        tuple(str(window) for window in study.window_sizes)
        if kind == "window"
        else tuple(runner.format_rate(rate) for rate in study.loss_rates)
    )
    getters = {name: path for name, path, _unit, _title in METRICS}
    rows: list[dict[str, Any]] = []
    for level in levels:
        for metric in PAIRED_METRICS:
            path = getters[metric]
            values = []
            for seed in study.seeds:
                left = indexed.get(("GoBackN", level, seed))
                right = indexed.get(("SelectiveRepeat", level, seed))
                if left is None or right is None:
                    continue
                values.append(float(_metric(left, path)) - float(_metric(right, path)))
            row = _paired_row(header, level, metric, values)
            rows.append(row)
    return rows


def _paired_row(header: str, level: str, metric: str, values: Sequence[float]) -> dict[str, Any]:
    if not values:
        return {
            "comparison": "GoBackN - SelectiveRepeat",
            header: level,
            "metric": metric,
            "n": 0,
            "mean_difference": "",
            "std_difference": "",
            "sem_difference": "",
            "ci95_low": "",
            "ci95_high": "",
            "t_statistic": "",
            "exploratory_p": "",
        }
    low, high = stats.confidence_interval_95(values)
    deviation = stats.sample_std(values)
    error = stats.standard_error(values)
    t_stat = ""
    p_value = ""
    if error not in (None, 0.0) and len(values) >= 2:
        statistic = stats.mean(values) / error
        probability = min(1.0, 2.0 * stats.student_t_upper_tail(abs(statistic), len(values) - 1))
        t_stat = f"{statistic:.6g}"
        p_value = f"{probability:.6g}"
    return {
        "comparison": "GoBackN - SelectiveRepeat",
        header: level,
        "metric": metric,
        "n": len(values),
        "mean_difference": stats.mean(values),
        "std_difference": "" if deviation is None else deviation,
        "sem_difference": "" if error is None else error,
        "ci95_low": "" if low is None else low,
        "ci95_high": "" if high is None else high,
        "t_statistic": t_stat,
        "exploratory_p": p_value,
    }


def is_primary_counterpart(record: Mapping[str, Any]) -> bool:
    """True for the cells whose controls match the primary study at reorder 0."""

    if float(record["forward_config"]["configured_loss_rate"]) != 0.0:
        return False
    if float(record["forward_config"]["configured_reorder_rate"]) != 0.0:
        return False
    protocol = str(record["meta"]["protocol"])
    window = int(record["meta"]["window_size"])
    if protocol == "StopAndWait":
        return window == 1
    return window == 8


def primary_counterparts(records: Sequence[Mapping[str, Any]]) -> dict[tuple[str, int, int], Mapping[str, Any]]:
    indexed: dict[tuple[str, int, int], Mapping[str, Any]] = {}
    for record in records:
        if float(record["forward_config"]["configured_reorder_rate"]) != 0.0:
            continue
        if float(record["forward_config"]["configured_loss_rate"]) != 0.0:
            continue
        key = (
            str(record["meta"]["protocol"]),
            int(record["meta"]["seed"]),
            int(record["meta"]["window_size"]),
        )
        if key in indexed:
            raise SystemExit(f"duplicate primary reorder-0 record {key}")
        indexed[key] = record
    return indexed


def regression_rows(
    new_records: Sequence[Mapping[str, Any]],
    primary_records: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    """Compare identical-configuration cells against the frozen primary reorder-0 rows.

    This checks reproducibility of the same controls. It does not assert that
    Go-Back-N and Selective Repeat produce the same measurements.
    """

    primary = primary_counterparts(primary_records)
    rows: list[dict[str, Any]] = []
    for record in new_records:
        if not is_primary_counterpart(record):
            continue
        key = (
            str(record["meta"]["protocol"]),
            int(record["meta"]["seed"]),
            int(record["meta"]["window_size"]),
        )
        baseline = primary.get(key)
        for path in REGRESSION_FIELDS:
            field = ".".join(path)
            new_value = "" if baseline is None else _metric(record, path)
            if baseline is None:
                old_value = ""
                matched = False
            else:
                old_value = _metric(baseline, path)
                new_value = _metric(record, path)
                matched = _values_match(new_value, old_value)
            rows.append(
                {
                    "experiment_id": record["meta"]["experiment_id"],
                    "protocol": key[0],
                    "seed": key[1],
                    "window_size": key[2],
                    "field": field,
                    "new_value": new_value,
                    "primary_value": old_value,
                    "match": matched,
                }
            )
    return rows


def _values_match(left: Any, right: Any) -> bool:
    if isinstance(left, bool) or isinstance(right, bool):
        return left is right
    if isinstance(left, (int, float)) and isinstance(right, (int, float)):
        return float(left) == float(right)
    return left == right


def write_table(path: Path, rows: Sequence[Mapping[str, Any]], fieldnames: Sequence[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(fieldnames))
        writer.writeheader()
        for row in rows:
            writer.writerow({key: "" if row.get(key) is None else row.get(key) for key in fieldnames})


def _svg_escape(text: str) -> str:
    return (
        text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )


def write_factor_svg(
    path: Path,
    *,
    title: str,
    xlabel: str,
    ylabel: str,
    series: Sequence[Mapping[str, Any]],
    tick,
) -> None:
    width, height = 860, 520
    left, right, top, bottom = 100, 40, 70, 80
    plot_w = width - left - right
    plot_h = height - top - bottom
    xs = sorted({x for item in series for x in item["xs"]})
    ys: list[float] = []
    for item in series:
        ys.extend(item["ys"])
        ys.extend(y for y in item["lows"] if y is not None)
        ys.extend(y for y in item["highs"] if y is not None)
    y_min = min(ys) if ys else 0.0
    y_max = max(ys) if ys else 1.0
    if y_max == y_min:
        y_max = y_min + 1.0
    pad = 0.08 * (y_max - y_min)
    y_min -= pad
    y_max += pad

    def sx(value: float) -> float:
        if len(xs) == 1:
            return left + plot_w / 2
        return left + (xs.index(value) / (len(xs) - 1)) * plot_w

    def sy(value: float) -> float:
        return top + (y_max - value) / (y_max - y_min) * plot_h

    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
        '<rect width="100%" height="100%" fill="#ffffff"/>',
        f'<text x="{width / 2}" y="28" text-anchor="middle" font-family="sans-serif" font-size="16">{_svg_escape(title)}</text>',
        f'<line x1="{left}" y1="{top + plot_h}" x2="{left + plot_w}" y2="{top + plot_h}" stroke="#222"/>',
        f'<line x1="{left}" y1="{top}" x2="{left}" y2="{top + plot_h}" stroke="#222"/>',
        f'<text x="{left + plot_w / 2}" y="{height - 16}" text-anchor="middle" font-family="sans-serif" font-size="13">{_svg_escape(xlabel)}</text>',
        f'<text x="18" y="{top + plot_h / 2}" text-anchor="middle" font-family="sans-serif" font-size="13" transform="rotate(-90 18 {top + plot_h / 2})">{_svg_escape(ylabel)}</text>',
    ]
    for x_value in xs:
        px = sx(x_value)
        parts.append(
            f'<text x="{px:.1f}" y="{top + plot_h + 22}" text-anchor="middle" font-family="sans-serif" font-size="11">{_svg_escape(tick(x_value))}</text>'
        )
    legend_y = 48
    for index, item in enumerate(series):
        color = item["color"]
        points = []
        for x_value, y_value, low, high in zip(item["xs"], item["ys"], item["lows"], item["highs"]):
            px, py = sx(x_value), sy(y_value)
            points.append(f"{px:.1f},{py:.1f}")
            if low is not None and high is not None:
                parts.append(
                    f'<line x1="{px:.1f}" y1="{sy(low):.1f}" x2="{px:.1f}" y2="{sy(high):.1f}" stroke="{color}" stroke-width="1.5"/>'
                )
            parts.append(f'<circle cx="{px:.1f}" cy="{py:.1f}" r="4" fill="{color}"/>')
        if points:
            parts.append(
                f'<polyline points="{" ".join(points)}" fill="none" stroke="{color}" stroke-width="2"/>'
            )
        lx = left + index * 190
        parts.append(f'<rect x="{lx}" y="{legend_y}" width="14" height="14" fill="{color}"/>')
        parts.append(
            f'<text x="{lx + 20}" y="{legend_y + 12}" font-family="sans-serif" font-size="12">{_svg_escape(item["name"])}</text>'
        )
    parts.append("</svg>")
    path.write_text("\n".join(parts) + "\n", encoding="utf-8")


def series_for_metric(kind: str, metric_rows: Sequence[Mapping[str, Any]], metric: str) -> list[dict[str, Any]]:
    header = factor_header(kind)
    series = []
    for protocol in PROTOCOLS:
        rows = [
            row
            for row in metric_rows
            if row["protocol"] == protocol and row["metric"] == metric and int(row["n"]) > 0
        ]
        rows.sort(key=lambda row: float(row[header]))
        series.append(
            {
                "name": protocol,
                "color": PROTOCOL_COLOR[protocol],
                "xs": [float(row[header]) for row in rows],
                "ys": [float(row["mean"]) for row in rows],
                "lows": [None if row["ci95_low"] == "" else float(row["ci95_low"]) for row in rows],
                "highs": [None if row["ci95_high"] == "" else float(row["ci95_high"]) for row in rows],
            }
        )
    return series


def write_plots(kind: str, metric_rows: Sequence[Mapping[str, Any]], out: Path) -> list[Path]:
    suffix = "window" if kind == "window" else "loss"
    xlabel = factor_axis_label(kind)
    tick = (lambda value: str(int(value))) if kind == "window" else (lambda value: f"{value:.2f}")
    specs = [
        (
            f"goodput_vs_{suffix}.svg",
            "goodput_bytes_sec",
            f"Mean goodput vs {suffix}",
            "Mean goodput (bytes/s)",
        ),
        (
            f"data_retransmissions_vs_{suffix}.svg",
            "data_retransmissions",
            f"Mean data retransmissions vs {suffix}",
            "Mean data retransmissions (packets)",
        ),
        (
            f"duration_vs_{suffix}.svg",
            "duration_secs",
            f"Mean transfer duration vs {suffix}",
            "Mean virtual duration (s)",
        ),
    ]
    written = []
    for filename, metric, title, ylabel in specs:
        path = out / filename
        write_factor_svg(
            path,
            title=title,
            xlabel=xlabel,
            ylabel=ylabel,
            series=series_for_metric(kind, metric_rows, metric),
            tick=tick,
        )
        written.append(path)
    return written


def _fmt(value: Any) -> str:
    if value == "" or value is None:
        return ""
    if isinstance(value, float):
        return f"{value:.6g}"
    return str(value)


def write_findings(
    kind: str,
    path: Path,
    *,
    record_count: int,
    failures: Sequence[Mapping[str, Any]],
    metric_rows: Sequence[Mapping[str, Any]],
    paired: Sequence[Mapping[str, Any]],
    regression: Sequence[Mapping[str, Any]],
) -> None:
    study = study_for(kind)
    header = factor_header(kind)
    mismatches = [row for row in regression if row["match"] is not True]
    lines = [
        f"# {'Window-size' if kind == 'window' else 'Loss-rate'} goodput study",
        "",
        "These numbers are computed from the Rust records in this directory.",
        "Intervals are two-sided 95% Student-t intervals on the successful trials in each cell.",
        "With five seeds, df = 4 when every seed succeeded. The interval is not truncated at zero.",
        "Paired rows are Go-Back-N minus Selective Repeat on the shared seed.",
        "Exploratory p-values are uncorrected. A small p-value is not treated as proof that one protocol is better.",
        "Agreement between protocols is reported as an observation. It is not a pass condition.",
        "",
        "## Design",
        "",
        f"- Study: `{study.name}`",
        f"- Records loaded: {record_count}",
        f"- Seeds: {', '.join(str(seed) for seed in study.seeds)}",
        f"- File chunk: {study.chunk_size} bytes",
        f"- Reorder: {study.reorder_rates[0]}",
        f"- RTO multiplier: {study.rto_multiplier}",
        f"- min/initial/max RTO: {study.min_rto_ms} / {study.initial_rto_ms} / {study.max_rto_ms} ms",
        f"- Delay / jitter / reorder hold: {study.base_delay_ms} / {study.jitter_ms} / {study.reorder_extra_ms} ms",
    ]
    if kind == "window":
        lines.append(f"- Windows for Go-Back-N and Selective Repeat: {', '.join(str(w) for w in study.window_sizes)}")
        lines.append("- Stop-and-Wait is included only at window 1.")
        lines.append("- Loss, duplication, and corruption are 0.")
    else:
        lines.append(f"- Loss rates: {', '.join(runner.format_rate(rate) for rate in study.loss_rates)}")
        lines.append("- Go-Back-N and Selective Repeat use window 8. Stop-and-Wait uses window 1.")
        lines.append("- Reorder, duplication, and corruption are 0.")
    lines.extend(["", "## Data quality", ""])
    if failures:
        lines.append(f"Non-success or hash-mismatch trials: {len(failures)}. They are excluded from means.")
        for record in failures:
            lines.append(
                f"- `{record['meta']['experiment_id']}` status={record['app']['transfer_status']} sha256_match={record['app']['sha256_match']}"
            )
    else:
        lines.append("Every loaded trial has transfer_status SUCCESS and sha256_match true.")
    lines.extend(["", "## Cell means", ""])
    lines.append(f"| protocol | {header} | metric | n | mean | 95% CI |")
    lines.append("|---|---|---|---|---|---|")
    for row in metric_rows:
        low = _fmt(row["ci95_low"])
        high = _fmt(row["ci95_high"])
        interval = "" if low == "" else f"[{low}, {high}]"
        lines.append(
            f"| {row['protocol']} | {row[header]} | {row['metric']} | {row['n']} | {_fmt(row['mean'])} | {interval} |"
        )
    lines.extend(["", "## Paired Go-Back-N minus Selective Repeat", ""])
    lines.append(f"| {header} | metric | n | mean difference | 95% CI | exploratory p |")
    lines.append("|---|---|---|---|---|---|")
    for row in paired:
        low = _fmt(row["ci95_low"])
        high = _fmt(row["ci95_high"])
        interval = "" if low == "" else f"[{low}, {high}]"
        lines.append(
            f"| {row[header]} | {row['metric']} | {row['n']} | {_fmt(row['mean_difference'])} | {interval} | {row['exploratory_p']} |"
        )
    lines.extend(["", "## Regression against the frozen primary reorder-0 cells", ""])
    lines.append(
        "Compared fields: transfer status, SHA-256 match, delivered digest, data retransmissions, premature retransmissions, timeout count, virtual duration, and goodput."
    )
    lines.append(
        "Only cells with loss 0, reorder 0, and the primary window (1 for Stop-and-Wait, 8 otherwise) are compared. The primary file is read and not rewritten."
    )
    if not regression:
        lines.append("No counterpart cells were found, so the regression check did not run.")
    elif mismatches:
        lines.append(f"Mismatches: {len(mismatches)}.")
        for row in mismatches:
            lines.append(
                f"- `{row['experiment_id']}` {row['field']}: new={row['new_value']!r} primary={row['primary_value']!r}"
            )
    else:
        lines.append(f"All {len(regression)} compared field values match the frozen primary records.")
    lines.extend(
        [
            "",
            "## Limitations",
            "",
            "- Virtual-time driver, not the localhost UDP path.",
            "- Five seeds per cell. Intervals can be wide.",
            "- The window study does not cross window size with loss. The loss study holds window size fixed.",
            "- Goodput is the value already stored on the Rust record (`goodput_bytes_sec`). This script does not redefine it.",
            "",
        ]
    )
    path.write_text("\n".join(lines), encoding="utf-8")


def analyse(kind: str) -> int:
    raw_dir = output_dir_for(kind)
    records = load_records(raw_dir / "experiments_raw.json")
    failures = validate_matrix(kind, records)
    metric_rows = aggregate_rows(kind, records)
    paired = paired_rows(kind, records)
    if not PRIMARY_JSON.is_file():
        raise SystemExit(f"frozen primary dataset missing: {PRIMARY_JSON}")
    regression = regression_rows(records, load_records(PRIMARY_JSON))
    out = raw_dir / "analysis"
    out.mkdir(parents=True, exist_ok=True)
    header = factor_header(kind)
    write_table(
        out / "summary.csv",
        metric_rows,
        ( "protocol", header, "metric", "n", "mean", "std", "sem", "ci95_low", "ci95_high"),
    )
    write_table(
        out / "paired_by_seed.csv",
        paired,
        (
            "comparison",
            header,
            "metric",
            "n",
            "mean_difference",
            "std_difference",
            "sem_difference",
            "ci95_low",
            "ci95_high",
            "t_statistic",
            "exploratory_p",
        ),
    )
    write_table(
        out / "regression_vs_primary.csv",
        regression,
        (
            "experiment_id",
            "protocol",
            "seed",
            "window_size",
            "field",
            "new_value",
            "primary_value",
            "match",
        ),
    )
    plots = write_plots(kind, metric_rows, out)
    findings = out / "findings.md"
    write_findings(
        kind,
        findings,
        record_count=len(records),
        failures=failures,
        metric_rows=metric_rows,
        paired=paired,
        regression=regression,
    )
    mismatches = [row for row in regression if row["match"] is not True]
    print(f"study: {kind}")
    print(f"records: {len(records)}")
    print(f"failures: {len(failures)}")
    print(f"regression_rows: {len(regression)}")
    print(f"regression_mismatches: {len(mismatches)}")
    print(f"findings: {findings}")
    for plot in plots:
        print(f"figure: {plot}")
    if failures:
        return 3
    if mismatches or not regression:
        return 2
    return 0


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Analyse the window or loss goodput study.")
    parser.add_argument("--study", required=True, choices=["window", "loss"])
    args = parser.parse_args(argv)
    return analyse(args.study)


if __name__ == "__main__":
    raise SystemExit(main())
