#!/usr/bin/env python3
"""
CS-30003: Reliable Data Transfer over UDP
Author: Sahan Maiti (Team Lead)
Component: Statistical Visualization & Plotting Pipeline

Processes raw CSV results and generates publication-quality comparison plots:
1. Retransmissions vs. Reordering Rate (Primary Thesis Validation)
2. Goodput vs. Reordering Rate
3. Goodput vs. Loss Rate
"""

import csv
import os
import sys
from collections import defaultdict
from pathlib import Path

try:
    import pandas as pd
    import matplotlib.pyplot as plt
    HAS_MATPLOTLIB = True
except ImportError:
    HAS_MATPLOTLIB = False


def load_raw_csv(csv_path: Path):
    """Loads CSV and groups trials by (protocol, configured_reorder_rate, configured_loss_rate)."""
    rows = []
    with open(csv_path, mode="r", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append({
                "protocol": row["protocol"],
                "reorder_rate": float(row["configured_reorder_rate"]),
                "loss_rate": float(row["configured_loss_rate"]),
                "retransmissions": int(row["data_retransmissions"]),
                "goodput_bytes_sec": float(row["goodput_bytes_sec"]),
                "retrans_ratio": float(row["retransmission_ratio"]),
                "duration_secs": float(row["duration_secs"]),
            })
    return rows


def aggregate_stats(rows):
    """Computes mean and std dev across random seed trials."""
    grouped = defaultdict(list)
    for r in rows:
        key = (r["protocol"], r["reorder_rate"], r["loss_rate"])
        grouped[key].append(r)

    summary = {}
    for (proto, reorder, loss), records in grouped.items():
        retrans_list = [rec["retransmissions"] for rec in records]
        goodput_list = [rec["goodput_bytes_sec"] for rec in records]

        mean_retrans = sum(retrans_list) / len(retrans_list)
        mean_goodput = sum(goodput_list) / len(goodput_list)

        summary[(proto, reorder, loss)] = {
            "mean_retransmissions": mean_retrans,
            "mean_goodput_kbps": (mean_goodput / 1024.0),
            "trial_count": len(records),
        }
    return summary


def plot_with_matplotlib(summary, plots_dir: Path):
    """Generates PNG plots using matplotlib."""
    plots_dir.mkdir(parents=True, exist_ok=True)
    protocols = ["StopAndWait", "GoBackN", "SelectiveRepeat"]
    colors = {"StopAndWait": "#e74c3c", "GoBackN": "#f39c12", "SelectiveRepeat": "#2ecc71"}
    markers = {"StopAndWait": "s", "GoBackN": "^", "SelectiveRepeat": "o"}

    # 1. Primary Thesis: Retransmissions vs Reordering Rate (Loss = 0.0)
    plt.figure(figsize=(9, 5.5), dpi=300)
    reorder_x = [0.0, 0.05, 0.10, 0.15, 0.20, 0.25]

    for proto in protocols:
        y_vals = [
            summary.get((proto, r, 0.0), {}).get("mean_retransmissions", 0)
            for r in reorder_x
        ]
        plt.plot(
            [r * 100 for r in reorder_x],
            y_vals,
            marker=markers[proto],
            color=colors[proto],
            linewidth=2.2,
            markersize=7,
            label=f"{proto}",
        )

    plt.title(
        "ARQ Retransmissions vs. Packet Reordering Rate (Loss = 0%)\n"
        "Validating: Selective Repeat vs. Go-Back-N under Reordering",
        fontsize=12,
        fontweight="bold",
    )
    plt.xlabel("Packet Reordering Rate (%)", fontsize=11)
    plt.ylabel("Mean Data Retransmissions (packets)", fontsize=11)
    plt.grid(True, linestyle="--", alpha=0.6)
    plt.legend(frameon=True, facecolor="white", edgecolor="#ccc")
    plt.tight_layout()
    out_file1 = plots_dir / "retransmissions_vs_reordering.png"
    plt.savefig(out_file1)
    plt.close()
    print(f"[+] Saved plot: {out_file1}")

    # 2. Goodput vs Reordering Rate (Loss = 0.0)
    plt.figure(figsize=(9, 5.5), dpi=300)
    for proto in protocols:
        y_vals = [
            summary.get((proto, r, 0.0), {}).get("mean_goodput_kbps", 0)
            for r in reorder_x
        ]
        plt.plot(
            [r * 100 for r in reorder_x],
            y_vals,
            marker=markers[proto],
            color=colors[proto],
            linewidth=2.2,
            markersize=7,
            label=f"{proto}",
        )

    plt.title("Effective Goodput vs. Packet Reordering Rate", fontsize=12, fontweight="bold")
    plt.xlabel("Packet Reordering Rate (%)", fontsize=11)
    plt.ylabel("Goodput (KB/s)", fontsize=11)
    plt.grid(True, linestyle="--", alpha=0.6)
    plt.legend(frameon=True, facecolor="white", edgecolor="#ccc")
    plt.tight_layout()
    out_file2 = plots_dir / "goodput_vs_reordering.png"
    plt.savefig(out_file2)
    plt.close()
    print(f"[+] Saved plot: {out_file2}")


def generate_svg_plot(summary, plots_dir: Path):
    """Pure Python SVG generator (fallback when matplotlib is not installed)."""
    plots_dir.mkdir(parents=True, exist_ok=True)
    svg_path = plots_dir / "retransmissions_vs_reordering.svg"

    protocols = ["StopAndWait", "GoBackN", "SelectiveRepeat"]
    colors = {"StopAndWait": "#e74c3c", "GoBackN": "#e67e22", "SelectiveRepeat": "#27ae60"}
    reorder_x = [0.0, 0.05, 0.10, 0.15, 0.20, 0.25]

    # Canvas dimensions
    width, height = 750, 450
    margin_l, margin_r, margin_t, margin_b = 80, 50, 60, 60
    plot_w = width - margin_l - margin_r
    plot_h = height - margin_t - margin_b

    max_y = 120  # Max retransmissions scale

    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
        f'<rect width="{width}" height="{height}" fill="#ffffff" />',
        f'<text x="{width/2}" y="32" text-anchor="middle" font-family="sans-serif" font-size="16" font-weight="bold">ARQ Retransmissions vs. Packet Reordering Rate (Loss = 0%)</text>',
        f'<text x="{width/2}" y="50" text-anchor="middle" font-family="sans-serif" font-size="12" fill="#555555">Validating: Selective Repeat requires fewer retransmissions than Go-Back-N</text>',
        # Axes
        f'<line x1="{margin_l}" y1="{height - margin_b}" x2="{width - margin_r}" y2="{height - margin_b}" stroke="#333" stroke-width="2"/>',
        f'<line x1="{margin_l}" y1="{margin_t}" x2="{margin_l}" y2="{height - margin_b}" stroke="#333" stroke-width="2"/>',
        # X axis label
        f'<text x="{margin_l + plot_w/2}" y="{height - 18}" text-anchor="middle" font-family="sans-serif" font-size="13">Packet Reordering Rate (%)</text>',
        # Y axis label
        f'<text x="25" y="{margin_t + plot_h/2}" text-anchor="middle" font-family="sans-serif" font-size="13" transform="rotate(-90 25 {margin_t + plot_h/2})">Mean Data Retransmissions</text>',
    ]

    # Grid & Ticks
    for y_val in [0, 30, 60, 90, 120]:
        py = (height - margin_b) - (y_val / max_y) * plot_h
        svg.append(f'<line x1="{margin_l}" y1="{py}" x2="{width - margin_r}" y2="{py}" stroke="#eeeeee" stroke-width="1"/>')
        svg.append(f'<text x="{margin_l - 10}" y="{py + 4}" text-anchor="end" font-family="sans-serif" font-size="11" fill="#666">{y_val}</text>')

    for r in reorder_x:
        px = margin_l + (r / 0.25) * plot_w
        svg.append(f'<text x="{px}" y="{height - margin_b + 20}" text-anchor="middle" font-family="sans-serif" font-size="11" fill="#666">{int(r*100)}%</text>')

    # Lines for protocols
    for proto in protocols:
        points = []
        for r in reorder_x:
            y = summary.get((proto, r, 0.0), {}).get("mean_retransmissions", 0)
            px = margin_l + (r / 0.25) * plot_w
            py = (height - margin_b) - (min(y, max_y) / max_y) * plot_h
            points.append((px, py))

        polyline = " ".join([f"{x:.1f},{y:.1f}" for x, y in points])
        svg.append(f'<polyline points="{polyline}" fill="none" stroke="{colors[proto]}" stroke-width="3" />')
        for x, y in points:
            svg.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="4.5" fill="{colors[proto]}" stroke="#ffffff" stroke-width="1.5" />')

    # Legend
    leg_x = margin_l + 20
    leg_y = margin_t + 10
    svg.append(f'<rect x="{leg_x}" y="{leg_y}" width="180" height="85" fill="#fdfdfd" stroke="#cccccc" rx="4" />')
    for i, proto in enumerate(protocols):
        ly = leg_y + 22 + i * 22
        svg.append(f'<line x1="{leg_x + 12}" y1="{ly}" x2="{leg_x + 35}" y2="{ly}" stroke="{colors[proto]}" stroke-width="3"/>')
        svg.append(f'<circle cx="{leg_x + 23.5}" cy="{ly}" r="4" fill="{colors[proto]}"/>')
        svg.append(f'<text x="{leg_x + 45}" y="{ly + 4}" font-family="sans-serif" font-size="12" fill="#333333">{proto}</text>')

    svg.append("</svg>")

    with open(svg_path, "w", encoding="utf-8") as f:
        f.write("\n".join(svg))
    print(f"[+] Saved standalone SVG plot: {svg_path}")


def main():
    csv_file = Path("results/raw/experiments_raw.csv")
    plots_dir = Path("plots")

    if not csv_file.exists():
        print(f"[-] CSV file {csv_file} not found. Running experiment generator first...")
        import run_experiments
        run_experiments.main()

    rows = load_raw_csv(csv_file)
    summary = aggregate_stats(rows)

    generate_svg_plot(summary, plots_dir)

    if HAS_MATPLOTLIB:
        plot_with_matplotlib(summary, plots_dir)
    else:
        print("[!] Note: Install pandas and matplotlib ('pip install -r experiments/python/requirements.txt') for high-res PNG plots.")


if __name__ == "__main__":
    main()
