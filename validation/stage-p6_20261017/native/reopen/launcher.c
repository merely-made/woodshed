#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <fcntl.h>
int main(void) {
if(chdir("/Users/markik/Code/repos/woodshed"))return 120;
if(setenv("WOODSHED_SCENARIO","/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/reopen/scenario.scn",1))return 121;
if(setenv("WOODSHED_CAPTURE_DIR","/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/reopen",1))return 121;
if(setenv("WOODSHED_RECEIPT","/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/reopen/scenario.done",1))return 121;
if(setenv("WOODSHED_PRESENTATION_TIMEOUT_SECS","60",1))return 121;
if(setenv("WOODSHED_WIDTH","1180",1))return 121;
if(setenv("WOODSHED_HEIGHT","1000",1))return 121;
if(setenv("TMPDIR","/tmp",1))return 121;
if(setenv("CAMBIUM_HOST_FRAME_TRACE","1",1))return 121;
if(setenv("WOODSHED_STATE","/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/profile/practice.bin",1))return 121;
if(setenv("WOODSHED_SETTINGS","/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/profile/settings.json",1))return 121;
if(setenv("GRAPHSHELL_APP_ENDPOINT","/tmp/ws-p6-1017-v1.sock",1))return 121;
if(setenv("XDG_DATA_HOME","/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/data",1))return 121;
if(setenv("XDG_CONFIG_HOME","/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/config",1))return 121;
if(setenv("XDG_CACHE_HOME","/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/cache",1))return 121;
if(setenv("MERE_ROOT","/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/mere",1))return 121;
int once=open("/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/reopen/launch.once",O_WRONLY|O_CREAT|O_EXCL,0600);if(once<0)return 0;close(once);
FILE *pid=fopen("/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/reopen/native.pid","w");if(pid){fprintf(pid,"%d\n",getpid());fclose(pid);}
int log=open("/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/reopen/native.log",O_WRONLY|O_CREAT|O_TRUNC,0600);if(log<0||dup2(log,1)<0||dup2(log,2)<0)return 122;
char *args[]={"/Users/markik/Code/testing/woodshed/stage-p6-20261017/native-v1/frozen/woodshed",NULL};execv(args[0],args);return 123;
}
