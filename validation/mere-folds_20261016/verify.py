from pathlib import Path
from collections import Counter
from PIL import Image, ImageStat
import hashlib,json,re,sys,subprocess
packet=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else Path(__file__).resolve().parent
base=packet/'native'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
points=[('seed','expanded'),('seed','one'),('seed','two'),('reopen','restored'),('reopen','summary-expanded'),('reopen','all-expanded')]
receipts=[json.loads((base/phase/('fold-'+stem+'.json')).read_text()) for phase,stem in points]
assert all(x['sealed'] and x['practice_saved'] and x['persisted_folds_match'] and x['persisted_owners_match'] for x in receipts)
for field in ['owner_digest','truth_digest','positions_digest']:
 assert len({x[field] for x in receipts})==1,(field,[(points[i],x[field]) for i,x in enumerate(receipts)])
assert receipts[2]['folds']==receipts[3]['folds'] and len(receipts[2]['folds'])==2
assert [len(x['folds']) for x in receipts]==[0,1,2,2,1,0]
rows=[]
for (phase,stem),x in zip(points,receipts):
 assert not x['rehearsal_running'] and not x['song_playing']
 assert x['presentation_version']==3
 owners={}
 for record in x['folds']:
  stand=record['stand_in']['Member']
  for m in record['members']:
   assert m not in owners
   owners[m]=stand
 hidden=sum(len(r['members'])-1 for r in x['folds'])
 assert len(x['visible_nodes'])==x['source_node_count']-hidden
 expected=Counter()
 for r in x['source_relations']:
  if not r['visible'] or (r['from'] not in owners and r['to'] not in owners):continue
  a,b=owners.get(r['from'],r['from']),owners.get(r['to'],r['to'])
  if a!=b:expected[a,b,r['kind']]+=1
 actual={}
 for bundle in x['boundary_bundles']:
  assert bundle['source_link_cleared'] and not bundle['route']
  actual[bundle['from'],bundle['to'],bundle['kind']]=int(bundle['label'].split()[0])
 assert dict(expected)==actual,(phase,stem,expected,actual)
 rows.append({'phase':phase,'checkpoint':stem,'folds':len(x['folds']),'visible_nodes':len(x['visible_nodes']),'source_nodes':x['source_node_count'],'source_relations':x['source_relation_count'],'boundary_bundles':len(actual),'boundary_source_cells':sum(actual.values()),'max_bundle_count':max(actual.values(),default=0)})
images=[]
for phase,expected in [('seed',5),('reopen',3)]:
 p=base/phase;process=json.loads((p/'process.json').read_text());assert process['success'] and process['launcher_exit']==0 and not process['timeout']
 pngs=sorted(p.glob('*.png'));assert len(pngs)==expected
 for f in pngs:
  im=Image.open(f).convert('RGB');std=ImageStat.Stat(im).stddev;assert min(im.size)>500 and max(std)>5
  images.append({'path':str(f.relative_to(base)),'sha256':sha(f),'size':im.size,'nonblank':True})
inputs=json.loads((packet/'inputs.json').read_text());root=Path(__file__).resolve().parents[2]
source_commit=inputs['woodshed_source']
assert all(hashlib.sha256(subprocess.check_output(['git','cat-file','blob',source_commit+':'+f],cwd=root)).hexdigest()==h for f,h in inputs['source_sha256'].items())
working_tree_still_matches=all(sha(root/f)==h for f,h in inputs['source_sha256'].items())
keeper=json.loads((packet/'keeper-process.json').read_text());assert keeper['exited'] and keeper['exit_code']==0 and keeper['original_files_unchanged']
result={'ok':True,'source_matches_recorded_commit':True,'working_tree_still_matches_tested_source':working_tree_still_matches,'exact_sealed_fold_reopen':True,'owner_truth_and_all_member_positions_unchanged':True,'native_actions_leave_processes_stopped':True,'boundary_counts_independently_recomputed':True,'checkpoints':rows,'captures':images,'keeper_exited_and_original_files_unchanged':True,'owner_digest':receipts[0]['owner_digest'],'truth_digest':receipts[0]['truth_digest'],'positions_digest':receipts[0]['positions_digest']}
(packet/'verification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='captures'},indent=2))
