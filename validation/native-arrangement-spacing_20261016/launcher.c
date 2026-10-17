#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <fcntl.h>
int main(void){
if(chdir("/Users/markik/Code/worktrees/woodshed-portable-fretboard"))return 120;
if(setenv("WOODSHED_SCENARIO","/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/frozen/native_arrangement_spacing.scn",1))return 121;
if(setenv("WOODSHED_CAPTURE_DIR","/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/spacing-v1",1))return 121;
if(setenv("WOODSHED_RECEIPT","/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/spacing-v1/scenario.done",1))return 121;
if(setenv("WOODSHED_PRESENTATION_TIMEOUT_SECS","60",1))return 121;
if(setenv("WOODSHED_WIDTH","1180",1))return 121;
if(setenv("WOODSHED_HEIGHT","1100",1))return 121;
if(setenv("WOODSHED_STATE","/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/profile/practice.bin",1))return 121;
if(setenv("WOODSHED_SETTINGS","/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/profile/settings.json",1))return 121;
if(setenv("WOODSHED_APPEARANCE_RECEIPT","1",1))return 121;
if(setenv("GRAPHSHELL_APP_ENDPOINT","/tmp/ws-spacing-20261016.sock",1))return 121;
if(setenv("TMPDIR","/tmp",1))return 121;
if(setenv("XDG_DATA_HOME","/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/data",1))return 121;
if(setenv("XDG_CONFIG_HOME","/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/config",1))return 121;
if(setenv("XDG_CACHE_HOME","/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/cache",1))return 121;
if(setenv("MERE_ROOT","/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/mere",1))return 121;
int once=open("/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/spacing-v1/launch.once",O_WRONLY|O_CREAT|O_EXCL,0600);if(once<0)return 0;close(once);
FILE *pid=fopen("/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/spacing-v1/native.pid","w");if(pid){fprintf(pid,"%d\n",getpid());fclose(pid);}
int log=open("/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/spacing-v1/native.log",O_WRONLY|O_CREAT|O_TRUNC,0600);if(log<0||dup2(log,1)<0||dup2(log,2)<0)return 122;
char *args[]={"/Users/markik/Code/testing/woodshed/arrangement-spacing-20261016/frozen/woodshed-genet",NULL};execv(args[0],args);return 123;}
