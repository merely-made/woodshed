"""Check recorded audit coverage against frozen Git blobs, not today's files.

This verifies reader attestations and source identity; it does not read code
semantically, run product tests, or prove an absence of defects.
"""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path


if not __debug__:
    raise RuntimeError("Coverage verification refuses optimized Python")


def load(path):
    return json.loads(path.read_text())


def covers(ranges, lines):
    cursor = 1
    for span in sorted(normalize(span) for span in ranges):
        start, end = span
        if start < 1 or end < start or end > lines:
            return False
        if start > cursor:
            return False
        cursor = max(cursor, end + 1)
    return cursor > lines


def normalize(span):
    if isinstance(span, dict):
        return int(span['start']), int(span['end'])
    if isinstance(span, str):
        return tuple(map(int, re.findall(r'\d+', span)))
    return tuple(map(int, span))


def verify(root, repo, overlay_root):
    inventory = load(root / 'inventory.json')
    revision = inventory['revision']
    expected = {row['path']: row for row in inventory['files']}
    assert len(expected) == len(inventory['files']), 'duplicate inventory path'
    seen = set()
    total_lines = 0
    partitions = []
    for lane in ['core', 'views', 'host', 'audio', 'shared', 'theory',
                 'hocket', 'redshank', 'ringdown', 'tooling', 'evidence']:
        assigned = load(root / (lane + '-assigned.json'))
        rows = load(root / (lane + '-coverage.json'))['files']
        by_path = {row['path']: row for row in rows}
        assert len(by_path) == len(rows), (lane, 'duplicate coverage row')
        assert set(by_path) == {row['path'] for row in assigned}, (lane, 'assignment mismatch')
        count_lines = 0
        for assignment in assigned:
            path = assignment['path']
            assert path not in seen and assignment == expected[path], (lane, path)
            row = by_path[path]
            data = subprocess.check_output(['git', 'show', f'{revision}:{path}'], cwd=repo)
            digest = hashlib.sha256(data).hexdigest()
            assert digest == assignment['sha256'] == row['sha256'], (path, 'hash')
            assert len(data.decode('utf-8').splitlines()) == assignment['lines'] == row['lines'], (path, 'lines')
            assert covers(row.get('read_ranges', row.get('full_file_read_ranges', [])), row['lines']), (path, 'range gap')
            seen.add(path)
            count_lines += row['lines']
        partitions.append({'lane': lane, 'files': len(assigned), 'lines': count_lines})
        total_lines += count_lines
    assert seen == set(expected), 'uncovered inventory paths'
    parent = load(root / 'parent-coverage.json')['files']
    for row in parent:
        data = subprocess.check_output(['git', 'show', f'{revision}:{row["path"]}'], cwd=repo)
        assert hashlib.sha256(data).hexdigest() == row['sha256']
        assert len(data.decode('utf-8').splitlines()) == row['lines']
        for span in row['read_ranges']:
            start, end = normalize(span)
            assert 1 <= start <= end <= row['lines'], (row['path'], 'invalid parent range')
        # Partial parent corroboration of a fully assigned file is not another
        # unique full-file read. Only omitted files add to the baseline total.
        if row['path'] in seen:
            continue
        assert row.get('status') == 'complete' and covers(row['read_ranges'], row['lines'])
        seen.add(row['path'])
        total_lines += row['lines']
    audit = load(root / 'audit-receipt.json')
    for artifact in audit['reports'] + [audit['summary']]:
        assert hashlib.sha256((root / artifact['path']).read_bytes()).hexdigest() == artifact['sha256'], artifact['path']
    overlay = load(root / 'local-overlay-coverage.json')
    overlay_count = 0
    overlay_lines = 0
    assignments = load(root / 'local-overlay-assigned.json')
    assert len(overlay['files']) == len(assignments) == 33
    by_path = {row['path']: row for row in overlay['files']}
    assert len(by_path) == 33
    for assignment in assignments:
        row = by_path[assignment['path']]
        assert row['sha256'] == assignment['sha256'] and row['lines'] == assignment['lines']
        assert row['whole_file_semantic_review'] and covers(row['semantic_review_ranges'], row['lines'])
        assert not row['unchanged_ranges_not_line_audited']
        data = (overlay_root / row['path']).read_bytes()
        assert hashlib.sha256(data).hexdigest() == row['sha256']
        assert len(data.decode('utf-8').splitlines()) == row['lines']
        overlay_count += 1
        overlay_lines += row['lines']
    assert len(seen) == 937 and total_lines == 231190
    assert overlay_lines == 34861
    return {'complete': True, 'frozen_revision': revision, 'files': len(seen),
            'lines': total_lines, 'partitions': partitions,
            'overlay_files': overlay_count, 'overlay_semantic_lines': overlay_lines,
            'overlapping_overlay_not_added_to_unique_total': True,
            'scope': 'Recorded collective reads and exact frozen inputs; not current-head full read or runtime qualification.'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--repository', type=Path, required=True)
    parser.add_argument('--overlay-source', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(verify(Path(__file__).resolve().parent,
                            args.repository, args.overlay_source), indent=2))
