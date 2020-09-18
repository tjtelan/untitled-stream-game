const uri = 'ws://' + location.host + '/ws';

let host_game_mode = document.getElementById("host_game_mode");
let join_game_mode = document.getElementById("join_game_mode");
let user_name = document.getElementById("user_name");
let room_code = document.getElementById("room_code");

// Landing
host_game_mode.onclick = function() {
    document.getElementById("landing").style.display = "none";
    document.getElementById("host_login").style.display = "block";
}

// Select Host a game
host_connect.onclick = function() {
    let ws = new WebSocket(uri);
    let connect_btn = document.getElementById('host_connect');
    let user_name_input = document.getElementById('host_user_name');
    let host_login_div = document.getElementById('host_login');
    let game_lobby_div = document.getElementById('connected_lobby');

    ws.onopen = function() {
        ws_connect_status.innerHTML = '<p><em>Connected!</em></p>';
        join_game_lobby(ws, "Host", user_name_input.value, "" );

        host_login_div.remove();
        game_lobby_div.style.display = "block";
        // TODO: Add a button for starting the game

    };

    ws.onmessage = function(msg) {
        receive_msg(msg.data);
    };

    ws.onclose = function() {
        ws_connect_status.getElementsByTagName('em')[0].innerText = 'Disconnected!';
    };
}


// Select Join a game
join_game_mode.onclick = function() {
    document.getElementById("landing").style.display = "none";
    document.getElementById("room_login").style.display = "block";
}

room_connect.onclick = function() {
    let ws = new WebSocket(uri);
    let connect_btn = document.getElementById('room_connect');
    let room_code_input = document.getElementById('room_code_input');
    let user_name_input = document.getElementById('player_name_input');
    let room_login_div = document.getElementById('room_login');
    let game_lobby_div = document.getElementById('connected_lobby');

    ws.onopen = function() {
        ws_connect_status.innerHTML = '<p><em>Connected!</em></p>';
        join_game_lobby(ws, "Player", user_name_input.value, room_code_input.value );

        connect_btn.remove();
        room_login_div.remove();
        game_lobby_div.style.display = "block";
    };

    ws.onmessage = function(msg) {
        receive_msg(msg.data);
    };

    ws.onclose = function() {
        ws_connect_status.getElementsByTagName('em')[0].innerText = 'Disconnected!';
    };
}


// TODO: The server will need to tag JSON responses in some way...
function receive_msg(data) {
    const line = document.createElement('p');
    line.innerText = data;
    ws_connect_status.appendChild(line);
}

function join_game_lobby(ws, user_type, user_name, room_code) {
    var login_info = {
//        "room_code" : room_code,
        "user_name" : user_name,
        "user_type" : user_type
    };

    if (room_code) {
        login_info["room_code"] = room_code;
    }

    // If room_code is given, add it to login_info

    ws.send(JSON.stringify(login_info));
    console.log("Sending" + JSON.stringify(login_info));
    //receive_msg('<You>: ' + login_info.room_code + ' ' + login_info.user_name);

}