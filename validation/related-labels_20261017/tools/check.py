from pathlib import Path
import subprocess,json,datetime,re,os,sys
B=Path(__file__).parent;out=B/'checks';label=sys.argv[1];cwd=sys.argv[2];cmd=sys.argv[3:]
start=datetime.datetime.now(datetime.timezone.utc).isoformat()
with (out/(label+'.log')).open('x') as f:r=subprocess.run(cmd,cwd=cwd,stdout=f,stderr=subprocess.STDOUT,env={**os.environ,'CARGO_BUILD_JOBS':'2'})
text=(out/(label+'.log')).read_text()
suites=[dict(zip(['passed','failed','ignored','measured','filtered_out'],map(int,m))) for m in re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',text)]
(out/(label+'.json')).write_text(json.dumps(dict(command=cmd,cwd=cwd,started_utc=start,ended_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),exit_code=r.returncode,suites=suites),indent=2)+'\n')
print(label,'exit',r.returncode,'suites',suites,flush=True)
if r.returncode:print(text[-5000:])
raise SystemExit(r.returncode)
