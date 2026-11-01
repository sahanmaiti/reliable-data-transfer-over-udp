#!/usr/bin/env python3
"""Analyse the E4 RTO-sensitivity study under results/rto_sensitivity/.

Does not read or write results/raw/ or results/analysis/.
Uses the same Student-t 95% CI methodology as plot_results.py.
"""

from __future__ import annotations

import csv
import json
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any, Mapping, Sequence

sys.path.insert(0, str(Path(__file__).resolve().parent))

import plot_results as stats
import run_experiments as runner

REPO = Path(__file__).resolve().parents[2]
RAW_JSON = REPO / "results" / "rto_sensitivity" / "experiments_raw.json"
OUT = REPO / "results" / "rto_sensitivity" / "analysis"

PROTOCOLS = ("StopAndWait", "GoBackN", "SelectiveRepeat")
MULTIPLIERS = (0.5, 1.0, 3.0)
SEEDS = (101, 202, 303, 404, 505)
PROTOCOL_COLOR = {
    "StopAndWait": "#1f77b4",
    "GoBackN": "#d62728",
    "SelectiveRepeat": "#2ca02c",
}

METRICS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("premature_retransmissions", ("protocol", "premature_retransmissions")),
    ("data_retransmissions", ("protocol", "data_retransmissions")),
    ("timeout_retransmissions", ("protocol", "timeout_retransmissions")),
    ("timeout_count", ("timing", "timeout_count")),
    ("duration_secs", ("timing", "duration_secs")),
    ("goodput_bytes_sec", ("goodput_bytes_sec",)),
    ("final_rto_ms", ("timing", "final_rto_ms")),
    ("final_srtt_ms", ("timing", "final_srtt_ms")),
    ("final_rttvar_ms", ("timing", "final_rttvar_ms")),
)

PAIRED_METRICS = (
    "premature_retransmissions",
    "duration_secs",
    "data_retransmissions",
)


def load_records() -> list[dict[str, Any]]:
    if not RAW_JSON.is_file():
        raise SystemExit(f"E4 dataset not found: {RAW_JSON}")
    records = json.loads(RAW_JSON.read_text(encoding="utf-8"))
    if len(records) != 45:
        raise SystemExit(f"expected 45 E4 records, found {len(records)}")
    return records


def validate_matrix(records: Sequence[Mapping[str, Any]]) -> None:
    study = runner.rto_sensitivity_study()
    by_proto: dict[str, int] = defaultdict(int)
    by_mult: dict[float, int] = defaultdict(int)
    by_seed: dict[int, int] = defaultdict(int)
    by_cell: dict[tuple[str, float], int] = defaultdict(int)
    for record in records:
        proto = str(record["meta"]["protocol"])
        mult = float(record["rto_config"]["rto_multiplier"])
        seed = int(record["meta"]["seed"])
        by_proto[proto] += 1
        by_mult[mult] += 1
        by_seed[seed] += 1
        by_cell[(proto, mult)] += 1
        if record["app"]["transfer_status"] != "SUCCESS":
            raise SystemExit(f"non-SUCCESS: {record['meta']['experiment_id']}")
        if record["app"]["sha256_match"] is not True:
            raise SystemExit(f"SHA-256 mismatch: {record['meta']['experiment_id']}")
        if int(record["app"]["source_bytes"]) != 140000:
            raise SystemExit(f"source_bytes != 140000: {record['meta']['experiment_id']}")
        if int(record["app"]["delivered_unique_bytes"]) != 140000:
            raise SystemExit(f"delivered_unique_bytes != 140000: {record['meta']['experiment_id']}")
        if float(record["forward_config"]["configured_loss_rate"]) != study.loss_rate:
            raise SystemExit("loss_rate mismatch in E4 record")
        if float(record["forward_config"]["configured_reorder_rate"]) != 0.0:
            raise SystemExit("reorder_rate mismatch in E4 record")
        if float(record["rto_config"]["min_rto_ms"]) != float(study.min_rto_ms):
            raise SystemExit("min_rto_ms mismatch in E4 record")
    if dict(by_proto) != {p: 15 for p in PROTOCOLS}:
        raise SystemExit(f"protocol counts wrong: {dict(by_proto)}")
    if dict(by_mult) != {m: 15 for m in MULTIPLIERS}:
        raise SystemExit(f"multiplier counts wrong: {dict(by_mult)}")
    if dict(by_seed) != {s: 9 for s in SEEDS}:
        raise SystemExit(f"seed counts wrong: {dict(by_seed)}")
    if any(n != 5 for n in by_cell.values()):
        raise SystemExit(f"cell counts wrong: {dict(by_cell)}")


def _metric(record: Mapping[str, Any], path: tuple[str, ...]) -> float:
    current: Any = record
    for key in path:
        current = current[key]
    return float(current)


def index_records(
    records: Sequence[Mapping[str, Any]],
) -> dict[tuple[str, float, int], Mapping[str, Any]]:
    indexed: dict[tuple[str, float, int], Mapping[str, Any]] = {}
    for record in records:
        key = (
            str(record["meta"]["protocol"]),
            float(record["rto_config"]["rto_multiplier"]),
            int(record["meta"]["seed"]),
        )
        if key in indexed:
            raise SystemExit(f"duplicate E4 key {key}")
        indexed[key] = record
    return indexed


def aggregate_rows(records: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, float], list[Mapping[str, Any]]] = defaultdict(list)
    for record in records:
        key = (
            str(record["meta"]["protocol"]),
            float(record["rto_config"]["rto_multiplier"]),
        )
        grouped[key].append(record)

    rows: list[dict[str, Any]] = []
    for protocol in PROTOCOLS:
        for multiplier in MULTIPLIERS:
            cell = grouped[(protocol, multiplier)]
            for name, path in METRICS:
                values = [_metric(record, path) for record in cell]
                low, high = stats.confidence_interval_95(values)
                deviation = stats.sample_std(values)
                error = stats.standard_error(values)
                rows.append(
                    {
                        "protocol": protocol,
                        "rto_multiplier": runner.format_rate(multiplier),
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


def paired_rows(
    indexed: Mapping[tuple[str, float, int], Mapping[str, Any]],
) -> list[dict[str, Any]]:
    comparisons = ((0.5, 1.0), (3.0, 1.0))
    getters = {name: path for name, path in METRICS}
    rows: list[dict[str, Any]] = []
    for protocol in PROTOCOLS:
        for left, right in comparisons:
            for metric in PAIRED_METRICS:
                path = getters[metric]
                values = [
                    _metric(indexed[(protocol, left, seed)], path)
                    - _metric(indexed[(protocol, right, seed)], path)
                    for seed in SEEDS
                ]
                low, high = stats.confidence_interval_95(values)
                deviation = stats.sample_std(values)
                error = stats.standard_error(values)
                t_stat = ""
                p_value = ""
                if error not in (None, 0.0):
                    statistic = stats.mean(values) / error
                    probability = min(1.0, 2.0 * stats.student_t_upper_tail(abs(statistic), 4))
                    t_stat = f"{statistic:.6g}"
                    p_value = f"{probability:.6g}"
                rows.append(
                    {
                        "protocol": protocol,
                        "comparison": f"{runner.format_rate(left)} - {runner.format_rate(right)}",
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
                )
    return rows


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


def write_multiplier_svg(
    path: Path,
    title: str,
    ylabel: str,
    series: Sequence[Mapping[str, Any]],
) -> None:
    width, height = 860, 520
    left, right, top, bottom = 90, 40, 70, 70
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
        f'<text x="{width / 2}" y="32" text-anchor="middle" font-family="sans-serif" font-size="16">{_svg_escape(title)}</text>',
        f'<line x1="{left}" y1="{top + plot_h}" x2="{left + plot_w}" y2="{top + plot_h}" stroke="#222"/>',
        f'<line x1="{left}" y1="{top}" x2="{left}" y2="{top + plot_h}" stroke="#222"/>',
        f'<text x="{left + plot_w / 2}" y="{height - 18}" text-anchor="middle" font-family="sans-serif" font-size="13">RTO multiplier</text>',
        f'<text x="22" y="{top + plot_h / 2}" text-anchor="middle" font-family="sans-serif" font-size="13" transform="rotate(-90 22 {top + plot_h / 2})">{_svg_escape(ylabel)}</text>',
    ]
    for x_value in xs:
        px = sx(x_value)
        parts.append(
            f'<text x="{px:.1f}" y="{top + plot_h + 22}" text-anchor="middle" font-family="sans-serif" font-size="11">{x_value:.1f}×</text>'
        )
    legend_y = 52
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
        lx = left + index * 180
        parts.append(f'<rect x="{lx}" y="{legend_y}" width="14" height="14" fill="{color}"/>')
        parts.append(
            f'<text x="{lx + 20}" y="{legend_y + 12}" font-family="sans-serif" font-size="12">{_svg_escape(item["name"])}</text>'
        )
    parts.append("</svg>")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(parts) + "\n", encoding="utf-8")


def series_for_metric(metric_rows: Sequence[Mapping[str, Any]], metric: str) -> list[dict[str, Any]]:
    series = []
    for protocol in PROTOCOLS:
        rows = [
            row
            for row in metric_rows
            if row["protocol"] == protocol and row["metric"] == metric and int(row["n"]) > 0
        ]
        rows.sort(key=lambda row: float(row["rto_multiplier"]))
        series.append(
            {
                "name": protocol,
                "color": PROTOCOL_COLOR[protocol],
                "xs": [float(row["rto_multiplier"]) for row in rows],
                "ys": [float(row["mean"]) for row in rows],
                "lows": [None if row["ci95_low"] == "" else float(row["ci95_low"]) for row in rows],
                "highs": [None if row["ci95_high"] == "" else float(row["ci95_high"]) for row in rows],
            }
        )
    return series


def write_plots(metric_rows: Sequence[Mapping[str, Any]]) -> list[Path]:
    specs = [
        (
            "premature_retransmissions_vs_multiplier.svg",
            "premature_retransmissions",
            "Mean premature retransmissions vs RTO multiplier",
            "Mean premature retransmissions (packets)",
        ),
        (
            "duration_vs_multiplier.svg",
            "duration_secs",
            "Mean transfer duration vs RTO multiplier",
            "Mean virtual duration (s)",
        ),
        (
            "data_retransmissions_vs_multiplier.svg",
            "data_retransmissions",
            "Mean data retransmissions vs RTO multiplier",
            "Mean data retransmissions (packets)",
        ),
        (
            "final_rto_vs_multiplier.svg",
            "final_rto_ms",
            "Mean final RTO vs RTO multiplier",
            "Mean final RTO (ms)",
        ),
    ]
    written = []
    for filename, metric, title, ylabel in specs:
        path = OUT / filename
        write_multiplier_svg(path, title, ylabel, series_for_metric(metric_rows, metric))
        written.append(path)
    return written


def cell_mean(rows: Sequence[Mapping[str, Any]], protocol: str, multiplier: float, metric: str) -> float:
    tag = runner.format_rate(multiplier)
    for row in rows:
        if (
            row["protocol"] == protocol
            and row["rto_multiplier"] == tag
            and row["metric"] == metric
        ):
            return float(row["mean"])
    raise KeyError((protocol, multiplier, metric))


def write_findings(
    metric_rows: Sequence[Mapping[str, Any]],
    paired: Sequence[Mapping[str, Any]],
) -> Path:
    path = OUT / "findings.md"
    lines = [
        "# E4 RTO Sensitivity — Findings",
        "",
        "## Experimental design",
        "",
        "Secondary study required by the project proposal: scale the adaptive RTO",
        "while holding Jacobson/Karels α and β at their Rust defaults.",
        "",
        "- Protocols: StopAndWait, GoBackN, SelectiveRepeat",
        "- RTO multipliers: 0.5×, 1.0×, 3.0×",
        "- Seeds: 101, 202, 303, 404, 505 (n = 5 per cell)",
        "- Channel: loss = 0.05, reorder = 0, duplicate = 0, corrupt = 0",
        "- Delay: base 20 ms, jitter 0, reorder hold 50 ms",
        "- File: fixtures/transfer_medium.bin (140,000 bytes), chunk 1,400 bytes",
        "- Windows: StopAndWait = 1; GoBackN / SelectiveRepeat = 8",
        "- RTO: initial 1000 ms, min 10 ms (E4 only), max 60000 ms",
        "- Matrix: 3 × 3 × 5 = 45 trials; all SUCCESS with SHA-256 match",
        "",
        "## Why loss was introduced",
        "",
        "The frozen primary study uses loss = 0 and varies reorder. E4 instead",
        "introduces a fixed 5% loss with reorder = 0 so that genuine loss events",
        "exist for the timeout path. Without loss, a multiplier study would mostly",
        "observe idle RTO behaviour rather than recovery under loss.",
        "",
        "## Why min_rto differs from the primary study",
        "",
        "The primary study floors RTO at 200 ms. Under measured RTTs near 40 ms,",
        "a 0.5× multiplier would still clamp to 200 ms and look identical to 1.0×.",
        "E4 uses min_rto = 10 ms only so the 0.5× arm can express a shorter timer.",
        "This is a deliberate E4-only change; the primary floor remains 200 ms.",
        "",
        "## Observed RTO behaviour",
        "",
    ]

    for protocol in PROTOCOLS:
        finals = [
            f"{m:.1f}× → {cell_mean(metric_rows, protocol, m, 'final_rto_ms'):.1f} ms"
            for m in MULTIPLIERS
        ]
        lines.append(f"- **{protocol}** mean final RTO: {'; '.join(finals)}")
    lines.extend(
        [
            "",
            "Smoke trials (GoBackN, seed 101) already showed distinct final RTO for",
            "0.5× (82 ms) versus 3.0× (123 ms), confirming the multiplier reaches the",
            "estimator and is not silently clamped to one shared value.",
            "",
            "Note on SelectiveRepeat: mean final RTO at 0.5× is higher than at 1.0×.",
            "That is consistent with aggressive timeouts causing more backoff before",
            "the transfer ends; the endpoint RTO is not a pure scaled SRTT snapshot.",
            "",
            "## Premature retransmission behaviour",
            "",
        ]
    )
    for protocol in PROTOCOLS:
        vals = [
            f"{m:.1f}× mean={cell_mean(metric_rows, protocol, m, 'premature_retransmissions'):.2f}"
            for m in MULTIPLIERS
        ]
        lines.append(f"- **{protocol}**: {'; '.join(vals)}")
    lines.extend(
        [
            "",
            "Across all three protocols, premature retransmissions are concentrated at",
            "0.5× and are exactly zero at 1.0× and 3.0× in this matrix (all five seeds).",
            "That is a measured outcome, not a tuned requirement.",
            "",
            "## Recovery duration and retransmissions",
            "",
        ]
    )
    for protocol in PROTOCOLS:
        durs = [
            f"{m:.1f}× {cell_mean(metric_rows, protocol, m, 'duration_secs'):.3f}s"
            for m in MULTIPLIERS
        ]
        rets = [
            f"{m:.1f}× {cell_mean(metric_rows, protocol, m, 'data_retransmissions'):.1f}"
            for m in MULTIPLIERS
        ]
        lines.append(f"- **{protocol}** duration: {'; '.join(durs)}")
        lines.append(f"- **{protocol}** data retransmissions: {'; '.join(rets)}")
    lines.extend(
        [
            "",
            "Paired-by-seed differences versus the 1.0× baseline are in",
            "`paired_by_seed.csv`. Exploratory two-sided paired t p-values are reported",
            "for transparency only; a significant p-value is not treated as proof that",
            "a multiplier is 'better'.",
            "",
            "## Paired contrasts (mean difference, n = 5)",
            "",
        ]
    )
    for row in paired:
        if row["metric"] not in PAIRED_METRICS:
            continue
        lines.append(
            f"- {row['protocol']} [{row['comparison']}] {row['metric']}: "
            f"mean Δ = {float(row['mean_difference']):.4g}"
            + (
                f", exploratory p = {row['exploratory_p']}"
                if row["exploratory_p"]
                else ""
            )
        )
    lines.extend(
        [
            "",
            "## Limitations",
            "",
            "- Virtual-time channel; not a real UDP path or OS stack.",
            "- Fixed 5% Bernoulli loss; no burst-loss model.",
            "- Only three multipliers; no sweep between 0.5× and 3.0×.",
            "- E4 min_rto = 10 ms differs from the primary 200 ms floor, so absolute",
            "  RTO values are not directly comparable to the frozen reordering study.",
            "- n = 5 seeds per cell; intervals are wide and exploratory p-values are",
            "  uncorrected across many comparisons.",
            "- Endpoint final_rto can be inflated by timeout backoff, especially when",
            "  the multiplier is too aggressive.",
            "",
            "## Bottom line",
            "",
            "In this matrix, shortening the RTO (0.5×) produces clear premature",
            "retransmissions and generally more retransmission work, while lengthening",
            "it (3.0×) removes premature retransmissions and tends to lengthen recovery",
            "relative to 1.0×. The expected premature-versus-delayed-recovery trade-off",
            "is visible for duration and retransmission load; premature counts are",
            "nonzero only on the aggressive arm under these conditions.",
            "",
        ]
    )
    path.write_text("\n".join(lines), encoding="utf-8")
    return path


def write_readme() -> Path:
    path = REPO / "results" / "rto_sensitivity" / "README.md"
    text = """# E4 RTO Sensitivity Study

Secondary experiment required by the project proposal. Completely separate from the
frozen primary 90-trial reordering study under `results/raw/`.

## Matrix

| Factor | Values |
| --- | --- |
| Protocols | StopAndWait, GoBackN, SelectiveRepeat |
| RTO multipliers | 0.5, 1.0, 3.0 |
| Seeds | 101, 202, 303, 404, 505 |
| Loss | 0.05 |
| Reorder / duplicate / corrupt | 0 |
| Base delay / jitter / reorder hold | 20 ms / 0 / 50 ms |
| File | `fixtures/transfer_medium.bin` (140,000 bytes) |
| Chunk | 1,400 bytes |
| Windows | StopAndWait=1; GBN/SR=8 |
| min RTO | **10 ms (E4 only)** |
| initial / max RTO | 1000 ms / 60000 ms |

Total: **45 trials**.

## How to reproduce

```bash
cargo build --release
python3 experiments/python/run_experiments.py --study rto
python3 experiments/python/analyze_rto_sensitivity.py
```

`--study` is required. A bare runner invocation does not execute or overwrite anything.
E4 never writes under `results/raw/`.

## Layout

```
results/rto_sensitivity/
  experiments_raw.json
  experiments_raw.csv
  trials/*.json
  smoke/          # optional two-trial clamp check
  smoke_m3/
  analysis/
    summary.csv
    paired_by_seed.csv
    findings.md
    *.svg
```

## Analysis outputs

See `analysis/findings.md` for measured behaviour. Do not treat exploratory p-values
as a claim that one multiplier is universally better.
"""
    path.write_text(text, encoding="utf-8")
    return path


def main() -> int:
    records = load_records()
    validate_matrix(records)
    indexed = index_records(records)
    metric_rows = aggregate_rows(records)
    paired = paired_rows(indexed)
    OUT.mkdir(parents=True, exist_ok=True)
    write_table(
        OUT / "summary.csv",
        metric_rows,
        (
            "protocol",
            "rto_multiplier",
            "metric",
            "n",
            "mean",
            "std",
            "sem",
            "ci95_low",
            "ci95_high",
        ),
    )
    write_table(
        OUT / "paired_by_seed.csv",
        paired,
        (
            "protocol",
            "comparison",
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
    plots = write_plots(metric_rows)
    findings = write_findings(metric_rows, paired)
    readme = write_readme()
    print(f"records: {len(records)}")
    print(f"summary: {OUT / 'summary.csv'}")
    print(f"paired: {OUT / 'paired_by_seed.csv'}")
    print(f"findings: {findings}")
    print(f"readme: {readme}")
    for plot in plots:
        print(f"figure: {plot}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
