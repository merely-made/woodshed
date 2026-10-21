from pathlib import Path
import json,subprocess,sys,plistlib
root=Path(__file__).resolve().parent
name=sys.argv[1]
if name not in ('seed','reopen'):raise SystemExit('seed or reopen required')
run=root/name
run.mkdir(exist_ok=False)
bundle=run/'WoodshedBarGroups.app';mac=bundle/'Contents/MacOS';mac.mkdir(parents=True)
repo='/Users/markik/Code/worktrees/woodshed-portable-fretboard'
env={
'WOODSHED_SCENARIO':str(root/'frozen'/('native_arrangement_bar_groups_reopen.scn' if name=='reopen' else 'native_arrangement_bar_groups.scn')),
'WOODSHED_CAPTURE_DIR':str(run),'WOODSHED_RECEIPT':str(run/'scenario.done'),
'WOODSHED_PRESENTATION_TIMEOUT_SECS':'60','WOODSHED_WIDTH':'1180','WOODSHED_HEIGHT':'1100',
'WOODSHED_STATE':str(root/'profile/practice.bin'),'WOODSHED_SETTINGS':str(root/'profile/settings.json'),
'WOODSHED_APPEARANCE_RECEIPT':'1','GRAPHSHELL_APP_ENDPOINT':'/tmp/ws-bar-groups-20261018-8d9c.sock',
'TMPDIR':'/tmp','XDG_DATA_HOME':str(root/'data'),'XDG_CONFIG_HOME':str(root/'config'),
'XDG_CACHE_HOME':str(root/'cache'),'MERE_ROOT':str(root/'mere')}
source='#include <unistd.h>\n#include <stdlib.h>\n#include <stdio.h>\n#include <fcntl.h>\nint main(void){\n'
source+='if(chdir('+json.dumps(repo)+'))return 120;\n'
for k,v in env.items():source+='if(setenv('+json.dumps(k)+','+json.dumps(v)+',1))return 121;\n'
source+='int once=open('+json.dumps(str(run/'launch.once'))+',O_WRONLY|O_CREAT|O_EXCL,0600);if(once<0)return 0;close(once);\n'
source+='FILE *pid=fopen('+json.dumps(str(run/'native.pid'))+',"w");if(pid){fprintf(pid,"%d\\n",getpid());fclose(pid);}\n'
source+='int log=open('+json.dumps(str(run/'native.log'))+',O_WRONLY|O_CREAT|O_TRUNC,0600);if(log<0||dup2(log,1)<0||dup2(log,2)<0)return 122;\n'
source+='char *args[]={'+json.dumps(str(root/'frozen/woodshed-genet'))+',NULL};execv(args[0],args);return 123;}\n'
(run/'launcher.c').write_text(source)
subprocess.run(['cc',str(run/'launcher.c'),'-o',str(mac/'launch')],check=True)
with (bundle/'Contents/Info.plist').open('wb') as f:plistlib.dump({'CFBundleExecutable':'launch','CFBundleIdentifier':'com.merely.woodshed.bargroups.'+name,'CFBundleName':'Woodshed Bar Groups','CFBundlePackageType':'APPL','NSHighResolutionCapable':True},f)
subprocess.run(['/usr/bin/open','-n',str(bundle)],check=True)
print(run)
