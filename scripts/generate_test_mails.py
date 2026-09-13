#!/usr/bin/env python3
"""
Generate test email files using a local llama_server instance (e.g. at http://0.0.0.0:11434).

Format of each generated file:
- Line 1: Subject
- Line 2: (Empty line)
- Line 3+: Mail body text

Features:
- Multi-threaded generation utilizing llama_server's parallel slots (default: 4 workers).
- ChatML prefill prompt to bypass reasoning tokens and ensure clean Subject + Body format.
- Diverse SoLaWi/farming topics pool for realistic content.
- Resumable: Skips already generated files.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import random
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

DEFAULT_SERVER_URL = "http://0.0.0.0:11434"
DEFAULT_COUNT = 1000
DEFAULT_CONCURRENCY = 4
DEFAULT_OUTPUT_DIR = "test-data"
DEFAULT_MAX_TOKENS = 180

TOPIC_CATEGORIES = [
    "Ernte-Update: Reichliche Tomatenernte steht diese Woche in den Depots bereit",
    "Kartoffelernte-Aktionstag am kommenden Samstag - bitte feste Handschuhe und Stiefel mitbringen",
    "Saisonales Gemüse: Kürbisse, Lauch und Karotten treffen frisch in den Kisten ein",
    "Arbeitseinsatz Folientunnel: Folien reparieren und Regenrinnen reinigen",
    "Depot-Mitteilung: Bitte leere Gemüsekisten und saubere Pfandkisten zeitnah zurückbringen",
    "Verschiebung der Abholzeiten aufgrund des bevorstehenden Feiertags",
    "Rezepttipps für die Verarbeitung von reichlich Zucchini, Mangold und Fenchel",
    "Einladung und Tagesordnung zur ordentlichen Mitgliederversammlung (MV)",
    "Traktor- und Maschinen-Reparatur-Workshop am Sonntagnachmittag",
    "Frostwarnung: Dringend Vliesabdeckung für die späten Salatbeete erforderlich",
    "Sommerfest auf dem Acker: Mitbring-Picknick, Live-Musik und Hofführung für Kinder",
    "Jät-Aktion im Zwiebel- und Möhrenbeet: Helfende Hände am Mittwochnachmittag gesucht",
    "Apfelernte und Saftpress-Wochenende auf der Streuobstwiese",
    "Vorbereitung des Lagergemüses: Rote Bete, Sellerie und Kürbisse einlagern",
    "Bewässerungsplan und Gießdienste während der aktuellen Trockenperiode",
    "Kompostpflege und Bodenaufbau: Workshop zur Kompostwirtschaft",
    "Willkommenstour und Einführung für neue SoLaWi-Mitglieder auf dem Hof",
    "Jungpflanzen-Verteilung für den eigenen Balkon- und Gartenanbau",
    "Nützlingseinsatz und biologischer Pflanzenschutz im Gewächshaus",
    "Erdbeerbeet pflegen: Unkraut jäten und Stroh ausbringen",
    "Eier-Abo und Rückgabe von Eierkartons an der Verteilstelle",
    "Koordination der Abholdienste und Fahrer für die Depots",
    "Winterpause und Grundreinigung der Verteilstellen vor dem Jahreswechsel",
    "Imker-Gruppe: Honigernte und Vorbereitung der Bienenstöcke auf den Winter",
    "Kräutergruppe: Kräuter sammeln, trocknen und Teemischungen abfüllen",
]

TONES = [
    "freundlich und gemeinschaftlich",
    "praktisch und organisatorisch",
    "kurz und dringend",
    "herzlich und ermutigend",
    "detailliert und informativ",
]


def build_prompt(idx: int) -> tuple[str, str]:
    """Return the full prompt and stop tokens for completion."""
    topic = TOPIC_CATEGORIES[idx % len(TOPIC_CATEGORIES)]
    tone = TONES[(idx // len(TOPIC_CATEGORIES)) % len(TONES)]
    seed = (idx * 37 + 101) % 10000

    prompt = (
        "<|im_start|>system\n"
        "Du bist ein aktives Mitglied einer Solidarischen Landwirtschaft (SoLaWi). "
        "Schreibe eine realistische E-Mail auf Deutsch an die Verteilerliste. "
        "Die erste Zeile muss der Betreff sein, beginnend mit 'Subject: '. "
        "Danach eine Leerzeile, gefolgt vom E-Mail-Text auf Deutsch. "
        "Schreibe ausschließlich auf Deutsch. "
        "Verwende keine Gedanken, kein 'To answer this', keine Markdown-Gedankenblöcke und keine Meta-Kommentare.<|im_end|>\n"
        f"<|im_start|>user\n"
        f"Thema: {topic}. Ton: {tone}. Variante #{seed}. Schreibe eine vollständige, lebendige E-Mail auf Deutsch (2 bis 4 Absätze).<|im_end|>\n"
        "<|im_start|>assistant\n"
        "Subject: "
    )
    return prompt, topic


def request_completion(server_url: str, prompt: str, max_tokens: int, temperature: float) -> str:
    """Send completion request to llama_server."""
    endpoint = f"{server_url.rstrip('/')}/completion"
    payload = {
        "prompt": prompt,
        "n_predict": max_tokens,
        "temperature": temperature,
        "stop": ["<|im_end|>", "<|endoftext|>", "###"],
    }
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        endpoint,
        data=data,
        headers={"Content-Type": "application/json"},
    )

    with urllib.request.urlopen(req, timeout=60) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        return res.get("content", "")


def clean_and_format_email(raw_content: str, default_subject: str) -> str:
    """Ensure the text starts with Subject: on line 1, blank line 2, and body on line 3+."""
    full_text = "Subject: " + raw_content.strip()

    lines = full_text.splitlines()
    if not lines:
        return f"Subject: {default_subject}\n\nKein Inhalt generiert."

    first_line = lines[0].strip()
    # Strip residual ChatML or markdown header tags
    first_line = first_line.replace("<|im_end|>", "").replace("<|endoftext|>", "").strip()
    if first_line.lower().startswith("subject:"):
        subj_text = first_line[len("subject:"):].strip()
    elif first_line.lower().startswith("betreff:"):
        subj_text = first_line[len("betreff:"):].strip()
    else:
        subj_text = first_line.strip()

    # Strip markdown emphasis like ***, **, *, quotes
    subj_text = subj_text.strip("*_\"'# ").strip()
    if not subj_text:
        subj_text = default_subject

    first_line = f"Subject: {subj_text}"

    # Collect and clean body lines
    body_lines = lines[1:]
    while body_lines and (not body_lines[0].strip() or all(c in "_-=" for c in body_lines[0].strip())):
        body_lines.pop(0)

    # Strip repeated subject or title at start of body if present
    if body_lines and (
        body_lines[0].strip().lower() == subj_text.lower()
        or body_lines[0].strip().lower().startswith("betreff:")
        or body_lines[0].strip().lower().startswith("subject:")
    ):
        body_lines.pop(0)
        while body_lines and not body_lines[0].strip():
            body_lines.pop(0)

    cleaned_body: list[str] = []
    for line in body_lines:
        cleaned_line = line.replace("<|im_end|>", "").replace("<|endoftext|>", "").strip()
        cleaned_body.append(cleaned_line)

    body_text = "\n".join(cleaned_body).strip()
    if not body_text:
        body_text = f"Mitteilung bezüglich: {default_subject}."
    else:
        # If the body ends abruptly mid-sentence, trim back to the last complete sentence
        last_punct = max(body_text.rfind("."), body_text.rfind("!"), body_text.rfind("?"))
        if last_punct > 50:
            body_text = body_text[:last_punct + 1]

    return f"{first_line}\n\n{body_text}\n"


def generate_single_email(
    idx: int,
    output_dir: Path,
    server_url: str,
    max_tokens: int,
    force: bool = False,
) -> tuple[int, bool, float, str]:
    """Generate and save a single email file."""
    file_path = output_dir / f"mail_{idx:04d}.txt"

    # Check if already generated and non-empty
    if not force and file_path.exists() and file_path.stat().st_size > 20:
        return idx, True, 0.0, "Already exists"

    prompt, topic_title = build_prompt(idx)
    temp = 0.65 + (idx % 5) * 0.05  # slight temperature variation: 0.65 - 0.85

    start_time = time.time()
    try:
        raw_content = request_completion(server_url, prompt, max_tokens, temp)
        formatted_email = clean_and_format_email(raw_content, topic_title)

        # Write to file
        temp_file = output_dir / f".tmp_mail_{idx:04d}.txt"
        with open(temp_file, "w", encoding="utf-8") as f:
            f.write(formatted_email)
        temp_file.replace(file_path)

        duration = time.time() - start_time
        return idx, False, duration, "OK"
    except Exception as e:
        duration = time.time() - start_time
        return idx, False, duration, f"Error: {e}"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate test email files using local llama_server."
    )
    parser.add_argument(
        "--count",
        "-c",
        type=int,
        default=DEFAULT_COUNT,
        help=f"Total number of emails to generate (default: {DEFAULT_COUNT})",
    )
    parser.add_argument(
        "--output-dir",
        "-o",
        type=Path,
        default=Path(DEFAULT_OUTPUT_DIR),
        help=f"Directory to store generated files (default: {DEFAULT_OUTPUT_DIR})",
    )
    parser.add_argument(
        "--server-url",
        "-s",
        type=str,
        default=DEFAULT_SERVER_URL,
        help=f"llama_server base URL (default: {DEFAULT_SERVER_URL})",
    )
    parser.add_argument(
        "--concurrency",
        "-j",
        type=int,
        default=DEFAULT_CONCURRENCY,
        help=f"Number of parallel workers (default: {DEFAULT_CONCURRENCY})",
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=DEFAULT_MAX_TOKENS,
        help=f"Maximum tokens per email (default: {DEFAULT_MAX_TOKENS})",
    )
    parser.add_argument(
        "--start-index",
        type=int,
        default=1,
        help="Starting index for file numbering (default: 1)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Regenerate files even if they already exist",
    )

    args = parser.parse_args()

    args.output_dir.mkdir(parents=True, exist_ok=True)

    # Verify server connectivity first
    print(f"Checking llama_server at {args.server_url}...")
    try:
        req = urllib.request.Request(f"{args.server_url.rstrip('/')}/v1/models")
        with urllib.request.urlopen(req, timeout=5) as resp:
            data = json.loads(resp.read().decode())
            model_name = data.get("data", [{}])[0].get("id", "unknown")
            print(f"Connected to llama_server. Loaded model: {model_name}")
    except Exception as e:
        print(f"Error connecting to llama_server at {args.server_url}: {e}", file=sys.stderr)
        return 1

    indices = list(range(args.start_index, args.start_index + args.count))
    total = len(indices)
    print(
        f"Starting generation of {total} emails (workers: {args.concurrency}, "
        f"output: {args.output_dir.resolve()})..."
    )

    completed = 0
    skipped = 0
    errors = 0
    total_start = time.time()

    with concurrent.futures.ThreadPoolExecutor(max_workers=args.concurrency) as executor:
        future_to_idx = {
            executor.submit(
                generate_single_email,
                idx,
                args.output_dir,
                args.server_url,
                args.max_tokens,
                args.force,
            ): idx
            for idx in indices
        }

        for future in concurrent.futures.as_completed(future_to_idx):
            idx, was_cached, duration, status = future.result()
            completed += 1

            if was_cached:
                skipped += 1
            elif status != "OK":
                errors += 1
                print(f"[{completed}/{total}] mail_{idx:04d}.txt -> {status}", file=sys.stderr)
            else:
                pct = (completed / total) * 100
                print(f"[{completed}/{total} - {pct:5.1f}%] mail_{idx:04d}.txt generated ({duration:.2f}s)")

    elapsed = time.time() - total_start
    print(
        f"\nFinished in {elapsed:.2f}s! "
        f"Generated: {total - skipped - errors}, Skipped (existing): {skipped}, Errors: {errors}."
    )
    return 0 if errors == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
