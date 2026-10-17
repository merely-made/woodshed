"""Read-only P7b evidence verification. Historical source comes from Git."""
from pathlib import Path
import hashlib,json,re,subprocess
ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
receipt=json.loads((HERE/'receipt.json').read_text())
revision=receipt.get('implementation_revision','bc6c460d158bf16174ecd42b7c9f663b7b8709ec')
for item in receipt['source_hashes']:
    data=subprocess.check_output(['git','show',revision+':'+item['path']],cwd=ROOT)
    assert hashlib.sha256(data).hexdigest()==item['sha256'],item['path']
gate=json.loads((HERE/'checks/candidate-gate.json').read_text())
assert gate['exit_code']==0 and gate['suites']
text=(HERE/'checks/candidate-gate.log').read_text()
assert 'test result: FAILED.' not in text and not re.search(r'^error(?:\[|:)',text,re.M)
assert re.search(r'test result: ok\.',text)
assert all(s['failed']==0 for s in gate['suites'])
assert sum(s['passed'] for s in gate['suites'] if not s['filtered_out'])==receipt['counts']['affected_gate_cases']
assert sum(s['passed'] for s in gate['suites'] if s['filtered_out'])==receipt['counts']['capture_child_invocations_separate']
for name in ['native-fixture-tests','native-fixture-build','hocket-shared-leaf','redshank-shared-leaf']:
    assert json.loads((HERE/'checks'/f'{name}.json').read_text())['exit_code']==0,name
assert json.loads((HERE/'checks/recipe-check.json').read_text())['exit_code']==101
if 'artifact_hashes' in receipt:
    for name,digest in receipt['artifact_hashes'].items():
        assert hashlib.sha256((HERE/name).read_bytes()).hexdigest()==digest,name
print(f"Verified {len(receipt['source_hashes'])} historical source bindings and affected gate {receipt['counts']['affected_gate_cases']} cases; native status: {receipt['native']}")

# Native evidence is bound to its own clean historical revision, not current HEAD.
N=HERE/'native-v2'
def read(name):return json.loads((N/name).read_text())
source=read('source.json')
assert source['worktree']==''
assert source['binaries']['woodshed']==receipt['desktop_binary_sha256']
assert hashlib.sha256((N/'source-hashes.json').read_bytes()).hexdigest()==source['source_hashes_sha256']
bindings=read('source-hashes.json')
assert len(bindings)==receipt['native_source_bindings']==335
for name,digest in bindings.items():
    assert hashlib.sha256(subprocess.check_output(['git','show',source['revision']+':'+name],cwd=ROOT)).hexdigest()==digest,name
for phase,expected in [('seed',6),('reopen',2)]:
    done=(N/phase/'scenario.done').read_text()
    assert done.startswith('RESULT ok\n') and f'{expected} captured, 0 blank' in done
    assert len(list((N/phase).glob('*.png')))==expected
    for name,digest in read(phase+'/phase-hashes.json').items():
        if name=='launch.once':continue # empty launch guard is not packaged
        assert hashlib.sha256((N/phase/name).read_bytes()).hexdigest()==digest,name
    launch=read(phase+'/launch.json')
    assert launch['source']==source
    assert launch['binary_sha256']==receipt['desktop_binary_sha256']
    assert hashlib.sha256((N/phase/'scenario.scn').read_bytes()).hexdigest()==launch['scenario_sha256']
    scenario='scenarios/stage_p7b_seconds_backing'+('_reopen' if phase=='reopen' else '')+'.scn'
    assert (N/phase/'scenario.scn').read_bytes()==subprocess.check_output(['git','show',source['revision']+':'+scenario],cwd=ROOT)
base=read('seed/p7b-baseline.json');retimed=read('seed/p7b-retimed.json')
for data in [base,retimed]:
    assert data['streams']==1 and data['input_unopened'] and data['sealed_exact']
    assert sorted(s['seconds'] for s in data['admitted']['segments'] if s['seconds'] is not None)==[.875,1.125]
before=next(s for s in base['admitted']['segments'] if s['id']['card']==2)
after=next(s for s in retimed['admitted']['segments'] if s['id']['card']==2)
assert (before['bpm'],before['beats'])==(180.,3) and (after['bpm'],after['beats'])==(137.,5)
assert before['phrase']==after['phrase']
prior=read('seed/p7b-reordered.json');refused=read('seed/p7b-duration-refused.json')
for key in ['admitted','backend_admitted','manifest','captures','wav_digest','wav_frames']:assert prior[key]==refused[key],key
assert refused['requested_ready'] and refused['admitted']!=refused['requested']
a=read('seed/p7b-final.json');b=read('reopen/p7b-reopened.json')
for key in ['set','owner','looper','saved_backing','manifest','captures','admitted','backend_admitted','requested','wav_frames','wav_digest']:assert a[key]==b[key],key
assert a['worker_wav_exact'] and read('reopen/p7b-reopenexport.json')['worker_wav_exact']
assert a['sealed_exact'] and b['sealed_exact'] and b['playing'] is False
assert a['captures'][0]['frames']==54000 and a['captures'][0]['seconds']==1.125 and a['captures'][0]['sealed_pcm_exact']
copy=read('reopen/expected-copy.json');assert copy['copied_after_seed_exit']
assert copy['sha256']==hashlib.sha256((N/'seed/p7b-final.json').read_bytes()).hexdigest()
assert (N/'seed/loop-export.wav').read_bytes()==(N/'reopen/loop-export.wav').read_bytes()==(N/'reference/loop-reference.wav').read_bytes()
assert json.loads((HERE/'checks/sealed-reference.json').read_text())['exit_code']==0
comparison=read('reference/comparison.json');assert comparison['profile_hashes_before']==comparison['profile_hashes_after'] and comparison['profile_unchanged']
assert read('reference/reference.json')['frames']==a['wav_frames']==156000
completion=read('completion.json')
assert completion['all_owned_processes_exited'] and completion['keeper_exited'] and completion['keeper_original_files_unchanged']
assert completion['actual_worker_wav_sha256']==hashlib.sha256((N/'seed/loop-export.wav').read_bytes()).hexdigest()
for review in ['parent-visual-review.json','independent-visual-review.json']:
    data=read(review)
    if review=='parent-visual-review.json':
        rows=data['reviewed_images'];assert not data['functional_blockers']
    else:
        rows=data['images'];assert data['conclusion']['all_images_nonblank'] and data['conclusion']['seconds_labels_readable']
        assert data['findings'] # Preserve visible layout/copy issues and waveform limits.
    assert len(rows)==8
    for row in rows:assert hashlib.sha256((N/row['path']).read_bytes()).hexdigest()==row['sha256']
print('Verified native source, 8 reviewed captures, preserved clocks, exact PCM reopen and actual worker/reference WAV parity.')
