#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <fcntl.h>
int main(void) {
if(chdir("/Users/markik/Code/repos/woodshed"))return 120;
if(setenv("WOODSHED_SCENARIO","/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/frozen/catalog_audition_admission.scn",1))return 121;
if(setenv("WOODSHED_CAPTURE_DIR","/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/seed",1))return 121;
if(setenv("WOODSHED_RECEIPT","/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/seed/scenario.done",1))return 121;
if(setenv("WOODSHED_PRESENTATION_TIMEOUT_SECS","60",1))return 121;
if(setenv("WOODSHED_WIDTH","1180",1))return 121;
if(setenv("WOODSHED_HEIGHT","900",1))return 121;
if(setenv("WOODSHED_STATE","/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/profile/practice.bin",1))return 121;
if(setenv("WOODSHED_SETTINGS","/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/profile/settings.json",1))return 121;
if(setenv("GRAPHSHELL_APP_ENDPOINT","/tmp/ws-cat-20261016-v4.sock",1))return 121;
if(setenv("TMPDIR","/tmp",1))return 121;
if(setenv("CAMBIUM_HOST_FRAME_TRACE","1",1))return 121;
if(setenv("CAMBIUM_ROOTSTOCK_TRACE_PRODUCER_REFUSAL","1",1))return 121;
if(setenv("XDG_DATA_HOME","/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/data",1))return 121;
if(setenv("XDG_CONFIG_HOME","/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/config",1))return 121;
if(setenv("XDG_CACHE_HOME","/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/cache",1))return 121;
if(setenv("MERE_ROOT","/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/mere",1))return 121;
int once=open("/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/seed/launch.once",O_WRONLY|O_CREAT|O_EXCL,0600);if(once<0)return 0;close(once);
FILE *pid=fopen("/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/seed/native.pid","w");if(pid){fprintf(pid,"%d\n",getpid());fclose(pid);}
int log=open("/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/seed/native.log",O_WRONLY|O_CREAT|O_TRUNC,0600);if(log<0||dup2(log,1)<0||dup2(log,2)<0)return 122;
char *args[]={"/Users/markik/Code/testing/woodshed/catalog-audition-20261016/native-v4/frozen/woodshed-genet",NULL};execv(args[0],args);return 123;
}
