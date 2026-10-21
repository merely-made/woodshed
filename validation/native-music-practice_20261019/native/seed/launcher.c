#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <fcntl.h>
int main(void){
if(chdir("/Users/markik/Code/worktrees/woodshed-arrangement-playback"))return 120;
if(setenv("WOODSHED_SCENARIO","/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/frozen/native_music_practice.scn",1))return 121;
if(setenv("WOODSHED_CAPTURE_DIR","/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/seed",1))return 121;
if(setenv("WOODSHED_RECEIPT","/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/seed/scenario.done",1))return 121;
if(setenv("WOODSHED_PRESENTATION_TIMEOUT_SECS","60",1))return 121;
if(setenv("WOODSHED_WIDTH","1180",1))return 121;
if(setenv("WOODSHED_HEIGHT","1100",1))return 121;
if(setenv("WOODSHED_STATE","/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/profile/practice.bin",1))return 121;
if(setenv("WOODSHED_SETTINGS","/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/profile/settings.json",1))return 121;
if(setenv("WOODSHED_APPEARANCE_RECEIPT","1",1))return 121;
if(setenv("WOODSHED_MUSIC_PRACTICE_RECEIPT","1",1))return 121;
if(setenv("GRAPHSHELL_APP_ENDPOINT","/tmp/woodshed-music-practice-v4-20261019.sock",1))return 121;
if(setenv("TMPDIR","/tmp",1))return 121;
if(setenv("XDG_DATA_HOME","/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/data",1))return 121;
if(setenv("XDG_CONFIG_HOME","/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/config",1))return 121;
if(setenv("XDG_CACHE_HOME","/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/cache",1))return 121;
if(setenv("MERE_ROOT","/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/mere",1))return 121;
int once=open("/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/seed/launch.once",O_WRONLY|O_CREAT|O_EXCL,0600);if(once<0)return 0;close(once);
FILE *pid=fopen("/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/seed/native.pid","w");if(pid){fprintf(pid,"%d\n",getpid());fclose(pid);}
int log=open("/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/seed/native.log",O_WRONLY|O_CREAT|O_TRUNC,0600);if(log<0||dup2(log,1)<0||dup2(log,2)<0)return 122;
char *args[]={"/Users/markik/Code/output/woodshed-arrangement-playback-20261019/native-run-v4/frozen/woodshed-genet",NULL};execv(args[0],args);return 123;}
