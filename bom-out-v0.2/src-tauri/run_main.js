const fs = require('fs');
const http = require('http');
const code = fs.readFileSync('./main.js','utf-8');
const postdata = JSON.stringify({ code : code });
const options ={
    hostname:'localhost',
    port:49620,
    path:'/execute',
    method:'Post',
    headers:{
        'Content-Type':'application/javascript; charset=utf-8'
    }
};
const req = http.request(options,(res) => {
    let body = '';
    res.on('data',(chunk) => {
        body += chunk;
    });
    res.on('end',() => {
        console.log('Response:',body);
    });
});
req.on('error',(e) => {
    console.log('Error:',e);
});
req.write(postdata);
req.end();