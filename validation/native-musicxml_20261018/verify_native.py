from pathlib import Path
import hashlib,json,subprocess,datetime
b=Path(__file__).resolve().parent;r=b/'native-run-v2'
def read(name,kind='seed'):return json.loads((r/kind/(name+'.json')).read_text())
def sets(session):
 bank=session['working_sets'];result={str(bank['active_id']):session['set']}
 result.update({str(x['id']):x['set'] for x in bank['inactive']});return result
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
for part in ['seed','reopen']:
 text=(r/part/'scenario.done').read_text();assert text.startswith('RESULT ok\n'),(part,text)
 assert 'FAIL:' not in text
checkpoints={n:read(n) for n in ['progression-initial','progression-preview','progression-picked','rhythm-initial','rhythm-edited','final']}
checkpoints['reopened']=read('reopened','reopen')
for name,d in checkpoints.items():
 assert not d['dirty'],name
 assert sets(d['live'])==sets(d['stored']),(name,'live/stored material')
a=checkpoints['progression-initial'];p=checkpoints['progression-preview'];picked=checkpoints['progression-picked']
assert a['stored']['set']==p['stored']['set'] and a['score']==p['score']
assert a['stored']['set']['cards']==picked['stored']['set']['cards']
assert a['stored']['set']['music']!=picked['stored']['set']['music']
initial=checkpoints['rhythm-initial'];edited=checkpoints['rhythm-edited'];final=checkpoints['final'];reopened=checkpoints['reopened']
assert initial['stored']['set']['cards']==edited['stored']['set']['cards']
assert initial['stored']['set']['music']!=edited['stored']['set']['music']
assert sets(edited['stored'])==sets(final['stored'])==sets(reopened['stored'])
assert edited['score']==reopened['score']
first=str(picked['stored']['working_sets']['active_id']);assert sets(final['stored'])[first]==picked['stored']['set']
export=r/'seed/exported.musicxml';assert export.read_text()==edited['stored']['set']['music']['musicxml']
schema=Path('/Users/markik/Code/output/woodshed-notation-study-20261018/musicxml.xsd')
p=subprocess.run(['xmllint','--nonet','--noout','--schema',str(schema),str(export)],text=True,capture_output=True);assert p.returncode==0,p.stderr
reader=subprocess.run([str(b/'model-check/target/debug/read'),str(export)],text=True,capture_output=True);assert reader.returncode==0,reader.stderr
(b/'native-export-reader.log').write_text(reader.stdout+p.stderr)
source=json.loads((r/'sources-before-build.json').read_text());after=json.loads((r/'sources-after-build.json').read_text());now=json.loads((r/'sources-after-run.json').read_text());assert source==after==now
captures={str(p.relative_to(r)):sha(p) for name in ['seed','reopen'] for p in sorted((r/name).glob('*.png'))};assert len(captures)==12,len(captures)
logs={'model-final.log':9,'renderer-final.log':7,'music-view-final.log':17,'set-music-tests.log':4,'native-music-test-final.log':1,'native-legacy-final.log':3,'native-persona-final.log':19}
for log,n in logs.items():assert f'test result: ok. {n} passed; 0 failed;' in (b/log).read_text(),log
fixture=json.loads((b/'export-qualification.json').read_text());assert fixture['sources']['/Users/markik/Code/worktrees/woodshed-portable-fretboard/crates/woodshedding/src/notation.rs']==sha(Path('/Users/markik/Code/worktrees/woodshed-portable-fretboard/crates/woodshedding/src/notation.rs'))
native_semantic=json.loads((b/'native-export-semantic.json').read_text());assert native_semantic['pass_counts']['pitched_staff_pairs']==11 and native_semantic['pass_counts']['rest_staff_pairs']==2
report={'date_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'status':'pass','scope':'Native Woodshed notation prototype; actual-source local Mere supplier cohort, two sealed native processes; not canonical-lock integration, general import, third-party engraving, full-piece playback, hardware audio, or full suites.','native_binary_sha256':sha(r/'frozen/woodshed-genet'),'stable_source_inputs':len(source),'checks':{'preview_does_not_author':True,'cards_preserved':True,'inactive_set_preserved':True,'exact_musicxml_reopened':True,'musical_reading_reopened':True,'exact_native_export_matches_save':True,'native_export_xsd':True,'native_export_independent_reader':True,'source_ledger_unchanged_across_build_and_run':True},'focused_test_counts':logs,'focused_total':sum(logs.values()),'captures':captures,'native_export_sha256':sha(export),'fixture_export_checks':fixture['pass_counts'],'native_export_semantic_checks':native_semantic['pass_counts'],'reviewed_capture_count':12,'checkpoints':{name:hashlib.sha256(json.dumps(d,sort_keys=True).encode()).hexdigest() for name,d in checkpoints.items()},'inputs':{str(p):sha(p) for p in [Path(__file__),r/'frozen/native_musicxml_editing.scn',r/'frozen/native_musicxml_reopen.scn',r/'launch_native.py',r/'sources-before-build.json',r/'sources-after-build.json',r/'sources-after-run.json']}}
(b/'native-qualification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report['checks'],indent=2));print('focused cases',sum(logs.values()))
