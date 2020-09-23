const uri = 'ws://' + location.host + '/ws';

let host_game_mode = document.getElementById("host_game_mode");
let join_game_mode = document.getElementById("join_game_mode");
let user_name = document.getElementById("user_name");
let room_code = document.getElementById("room_code");
let user_type = "Player";

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
    let host_start_btn = document.getElementById('host_start_game');

    let rock_btn = document.getElementById('rock_btn');
    let paper_btn = document.getElementById('paper_btn');
    let scissors_btn = document.getElementById('scissors_btn');

    ws.onopen = function() {
        ws_connect_status.innerHTML = '<p><em>Connected!</em></p>';
        user_type = "Host";
        join_game_lobby(ws, user_type, user_name_input.value, "" );

        host_login_div.remove();
        game_lobby_div.style.display = "block";
        // TODO: Add a button for starting the game

        display_username(user_name_input.value);

    };

    ws.onmessage = function(msg) {
        receive_msg(msg.data);
    };

    ws.onclose = function() {
        ws_connect_status.getElementsByTagName('em')[0].innerText = 'Disconnected!';
    };

    host_start_btn.onclick = function () {

        let req =JSON.stringify({ "HostStartGame": { "room_code": room_code.innerHTML }});
        console.log("Start game: " + req);
        ws.send(req);

    }

    rock_btn.onclick = function() {
        let req =JSON.stringify({ "PlayerHand": { "user_name": user_name.innerHTML, "room_code" : room_code.innerHTML, hand: "Rock" }});
        console.log("Sending hand: " + req);
        ws.send(req);
    }

    paper_btn.onclick = function() {
        let req =JSON.stringify({ "PlayerHand": { "user_name": user_name.innerHTML, "room_code" : room_code.innerHTML, hand: "Paper" }});
        console.log("Sending hand: " + req);
        ws.send(req);
    }

    scissors_btn.onclick = function() {
        let req =JSON.stringify({ "PlayerHand": { "user_name": user_name.innerHTML, "room_code" : room_code.innerHTML, hand: "Scissors" }});
        console.log("Sending hand: " + req);
        ws.send(req);
    }

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

    let rock_btn = document.getElementById('rock_btn');
    let paper_btn = document.getElementById('paper_btn');
    let scissors_btn = document.getElementById('scissors_btn');

    ws.onopen = function() {
        ws_connect_status.innerHTML = '<p><em>Connected!</em></p>';
        user_type = "Player";
        join_game_lobby(ws, user_type, user_name_input.value, room_code_input.value );

        connect_btn.remove();
        room_login_div.remove();
        game_lobby_div.style.display = "block";

        display_username(user_name_input.value);
    };

    ws.onmessage = function(msg) {
        receive_msg(msg.data);
    };

    ws.onclose = function() {
        ws_connect_status.getElementsByTagName('em')[0].innerText = 'Disconnected!';
    };

    rock_btn.onclick = function() {
        let req =JSON.stringify({ "PlayerHand": { "user_name": user_name.innerHTML, "room_code" : room_code.innerHTML, hand: "Rock" }});
        console.log("Sending hand: " + req);
        ws.send(req);
    }

    paper_btn.onclick = function() {
        let req =JSON.stringify({ "PlayerHand": { "user_name": user_name.innerHTML, "room_code" : room_code.innerHTML, hand: "Paper" }});
        console.log("Sending hand: " + req);
        ws.send(req);
    }

    scissors_btn.onclick = function() {
        let req =JSON.stringify({ "PlayerHand": { "user_name": user_name.innerHTML, "room_code" : room_code.innerHTML, hand: "Scissors" }});
        console.log("Sending hand: " + req);
        ws.send(req);
    }
}


function display_username(name) {
    let user = document.getElementById('user_name');
    user.innerHTML = name;
}

// TODO: The server will need to tag JSON responses in some way...
function receive_msg(data) {

    let parsed = JSON.parse(data);

    if (parsed["PartyUpdate"]) {

        let room_code = document.getElementById('room_code');
        let party_members = document.getElementById('party_members');
        let host_start_btn = document.getElementById('host_start_game');

        room_code.innerHTML = parsed["PartyUpdate"].room_code;
        party_members.innerHTML = parsed["PartyUpdate"].users;


        // If party size > 1, the host start button should appear
        if (user_type == "Host") {
            if (parsed["PartyUpdate"].users.length > 1) {
                host_start_btn.style.display = "block"
            }
            else {
                host_start_btn.style.display = "none"
            }
        }

    } else if (parsed["GameStart"]) {
        let game_controls = document.getElementById('active_game_controls');
        let host_start_btn = document.getElementById('host_start_game');

        // Display RPS controls
        game_controls.style.display = "block";

        // And the host's start game button should go away
        if (user_type == "Host") {
            host_start_btn.style.display = "none";
        }

        console.log("Host has started the game");
    } else if (parsed["ServerHand"]) {

        console.log("Server threw hand: " + parsed["ServerHand"].hand);

    } else {
        console.log("key check failed: " + data);
        console.log(typeof data);
    }



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


    if (user_type == "Host") {
        let req = JSON.stringify({ "HostNewGame" : login_info });
        ws.send(req);
        console.log("Sending" + req);
    } else if ((user_type == "Player")) {
        let req = JSON.stringify({ "UserLogin" : login_info });
        ws.send(req);
        console.log("Sending" + req);

    } else {
        console.log("Invalid user type: " + user_type);
    }



    // If room_code is given, add it to login_info

    //ws.send(JSON.stringify(login_info));
    //console.log("Sending" + JSON.stringify(login_info));

    //receive_msg('<You>: ' + login_info.room_code + ' ' + login_info.user_name);

}