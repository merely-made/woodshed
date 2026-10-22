#!/usr/bin/env python3
"""Explicit-endpoint public-fact control for an owned synthetic custody keeper."""
import base64, json, os, socket, struct, sys

def call(endpoint, request):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(10)
        connection.connect(endpoint)
        def send(value):
            body=json.dumps(value,separators=(',',':')).encode()
            connection.sendall(struct.pack('=I',len(body))+body)
        def read_exact(size):
            data=b''
            while len(data)<size:
                part=connection.recv(size-len(data))
                if not part: raise RuntimeError('custody connection closed')
                data+=part
            return data
        def receive():
            size=struct.unpack('=I',read_exact(4))[0]
            if size>1024*1024: raise RuntimeError('oversized custody frame')
            return json.loads(read_exact(size))
        send({'schema':'mere.graphshell/app-broker-hello/v2','app':'woodshed','route':'custody'})
        challenge=receive()
        assert challenge['type']=='challenge', challenge
        nonce=base64.urlsafe_b64encode(os.urandom(32)).decode().rstrip('=')
        send({'type':'connect','schema':'mere.graphshell/app-connect/v1','host_nonce':challenge['challenge']['host_nonce'],'client_nonce':nonce})
        connected=receive()
        assert connected['type']=='connected', connected
        send({'type':'call','id':1,'call':request})
        reply=receive()
        assert reply.get('id')==1, reply
        if 'Err' in reply.get('answer',{}): raise RuntimeError(reply['answer']['Err'])
        return reply

if __name__=='__main__':
    endpoint,action,*args=sys.argv[1:]
    if action=='create': request={'call':'create_profile','profile':args[0],'display_name':args[1]}
    elif action=='choose': request={'call':'choose_profile','profile':args[0]}
    elif action=='as_status': request={'call':'as_profile','profile':args[0],'inner':{'call':'status'}}
    else: request={'call':action}
    print(json.dumps(call(endpoint,request),indent=2))
