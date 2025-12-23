const app = Vue.createApp({
  data: () => ({
    message: "Hello Score Board !",
    game_title: "",
    team_top: "",
    team_bottom: "",
    game_inning: 0,
    last_inning: 9,
    top: false,
    first_base: false,
    second_base: false,
    third_base: false,
    ball_cnt: 0,
    strike_cnt: 0,
    out_cnt: 0,
    score_top: 0,
    score_bottom: 0,
    game_start: false,
    game_array: [],
    team_items: [],
    socket: null,
    restoredFromServer: false,
    // WebSocket reconnection
    connectionStatus: 'connecting',
    reconnectAttempts: 0,
    maxReconnectAttempts: 10,
    reconnectDelay: 1000,
    reconnectTimer: null,
    // Master/Slave control
    clientRole: null,  // null | 'master' | 'slave'
    clientId: null,
    masterClientId: null,
  }),
  created() {
    // Initialize WebSocket connection
    this.connectWebSocket();

    // Load configuration from init_data.json (initial load)
    this.loadConfiguration(false);

    // Set up Electron reload-config listener
    if (window.electronAPI?.onReloadConfig) {
      window.electronAPI.onReloadConfig(() => {
        console.log('Received reload-config event from Electron');
        // Force reload to update all values including game_title, team names
        this.loadConfiguration(true);
      });
    }
  },
  beforeUnmount() {
    // Clean up WebSocket and timers
    this.cancelReconnect();
    if (this.socket) {
      this.socket.close();
    }
  },
  watch: {
    // 監視するデータをまとめて指定
    boardData: {
      handler() {
        // Don't send updates if data was just restored from server (prevent infinite loop)
        if (this.restoredFromServer) {
          this.restoredFromServer = false;
          return;
        }
        this.updateBoard();
      },
      deep: true, // ネストされたオブジェクトも監視
    },
    // Debug: Watch connectionStatus changes
    connectionStatus(newVal, oldVal) {
      console.log(`[Vue Watch] connectionStatus changed: ${oldVal} -> ${newVal}`);
    },
    // Debug: Watch clientRole changes
    clientRole(newVal, oldVal) {
      console.log(`[Vue Watch] clientRole changed: ${oldVal} -> ${newVal}`);
    },
  },
  computed: {
    // watchで監視するための、ボードに関連するデータをまとめた算出プロパティ
    boardData() {
      return {
        game_title: this.game_title,
        team_top: this.team_top,
        team_bottom: this.team_bottom,
        game_inning: this.game_inning,
        top: this.top,
        first_base: this.first_base,
        second_base: this.second_base,
        third_base: this.third_base,
        ball_cnt: this.ball_cnt,
        strike_cnt: this.strike_cnt,
        out_cnt: this.out_cnt,
        score_top: this.score_top,
        score_bottom: this.score_bottom,
        last_inning: this.last_inning,
      };
    },
    // UI should be disabled if role is slave
    isOperationDisabled() {
      const result = this.clientRole === 'slave';
      console.log(`[Vue Computed] isOperationDisabled: ${result} (clientRole: ${this.clientRole})`);
      return result;
    },
    // Show master indicator
    isMaster() {
      const result = this.clientRole === 'master';
      console.log(`[Vue Computed] isMaster: ${result} (clientRole: ${this.clientRole})`);
      return result;
    },
  },
  methods: {
    /**
     * Load configuration from init_data.json
     * @param {boolean} forceReload - If true, reload all values including game_title, team names regardless of server state
     */
    loadConfiguration(forceReload = false) {
      fetch("/init_data.json")
        .then((response) => response.json())
        .then((data) => {
          // Always load UI configuration (dropdown options)
          this.game_array = data.game_array;
          this.team_items = data.team_items;

          // Load initial values if not restored from server OR if force reload requested
          if (forceReload || !this.restoredFromServer) {
            this.game_title = data.game_title;
            this.team_top = data.team_top;
            this.team_bottom = data.team_bottom;
            if (data.last_inning !== undefined) {
              this.last_inning = data.last_inning;
            }
          }

          const reloadType = forceReload ? 'forced' : 'initial';
          console.log(`Configuration loaded successfully (${reloadType})`);
        })
        .catch((error) => {
          console.error('Failed to load configuration:', error);
        });
    },

    // WebSocket connection management
    connectWebSocket() {
      // Dynamically generate WebSocket URL based on current page location
      // In Electron environment, always use localhost to ensure local server connection
      const isElectron = window.electronAPI?.isElectron || false;
      const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const wsHost = isElectron ? 'localhost:8080' : window.location.host;

      try {
        this.socket = new WebSocket(`${wsProtocol}//${wsHost}/ws`);

        this.socket.onopen = () => this.handleWebSocketOpen();
        this.socket.onmessage = (event) => this.handleWebSocketMessage(event);
        this.socket.onerror = (error) => this.handleWebSocketError(error);
        this.socket.onclose = () => this.handleWebSocketClose();
      } catch (error) {
        console.error("Failed to create WebSocket:", error);
        this.scheduleReconnect();
      }
    },

    handleWebSocketOpen() {
      console.log("WebSocket connection established for control panel.");
      console.log("  connectionStatus before:", this.connectionStatus);
      this.connectionStatus = 'connected';
      console.log("  connectionStatus after:", this.connectionStatus);
      this.reconnectAttempts = 0;

      // Get stored master token from sessionStorage
      const storedToken = sessionStorage.getItem('masterToken');
      console.log("  Stored master token:", storedToken ? "exists" : "none");

      // Send handshake to identify as operation client
      const handshakeMessage = {
        type: 'handshake',
        clientType: 'operation'
      };

      // Include token if available
      if (storedToken) {
        handshakeMessage.masterToken = storedToken;
        console.log('  Sending handshake with stored master token');
      } else {
        console.log('  Sending handshake without master token');
      }

      console.log("  Sending handshake:", JSON.stringify(handshakeMessage));
      this.socket.send(JSON.stringify(handshakeMessage));
    },

    handleWebSocketMessage(event) {
      console.log("Received WebSocket message:", event.data);
      try {
        const message = JSON.parse(event.data);
        console.log("  Parsed message type:", message.type);
        console.log("  Full message:", message);

        // Handle role assignment
        if (message.type === 'role_assignment') {
          console.log("  Processing role_assignment");
          this.clientRole = message.role;
          this.clientId = message.clientId;
          this.masterClientId = message.masterClientId;
          console.log(`    clientRole: ${this.clientRole}`);
          console.log(`    clientId: ${this.clientId}`);
          console.log(`    masterClientId: ${this.masterClientId}`);

          // Store master token if provided
          if (message.role === 'master' && message.masterToken) {
            sessionStorage.setItem('masterToken', message.masterToken);
            console.log('    Master token saved to sessionStorage');
          }

          console.log(`  Assigned role: ${message.role}`);
          return;
        }

        // Handle role change
        if (message.type === 'role_changed') {
          console.log("  Processing role_changed");
          this.clientRole = message.newRole;
          console.log(`    newRole: ${message.newRole}`);

          // Save new master token if provided
          if (message.newRole === 'master' && message.masterToken) {
            sessionStorage.setItem('masterToken', message.masterToken);
            console.log('    Master token saved to sessionStorage');
          }

          // Clear token if instructed
          if (message.clearToken) {
            sessionStorage.removeItem('masterToken');
            console.log('    Master token removed from sessionStorage');
          }

          console.log(`  Role changed to: ${message.newRole} (reason: ${message.reason})`);
          return;
        }

        // Handle game state update
        if (message.type === 'game_state' || !message.type) {
          const savedState = message.boardData || message.data || message;
          console.log("  Processing game_state");
          console.log("    Received game state from server:", savedState);

          // Restore game state (but not UI configuration like game_array and team_items)
          this.game_title = savedState.game_title || this.game_title;
          this.team_top = savedState.team_top || this.team_top;
          this.team_bottom = savedState.team_bottom || this.team_bottom;
          this.game_inning = savedState.game_inning || 0;
          this.last_inning = savedState.last_inning || 9;
          this.top = savedState.top || false;
          this.first_base = savedState.first_base || false;
          this.second_base = savedState.second_base || false;
          this.third_base = savedState.third_base || false;
          this.ball_cnt = savedState.ball_cnt || 0;
          this.strike_cnt = savedState.strike_cnt || 0;
          this.out_cnt = savedState.out_cnt || 0;
          this.score_top = savedState.score_top || 0;
          this.score_bottom = savedState.score_bottom || 0;

          this.restoredFromServer = true;
          console.log("    Game state restored");
        }
      } catch (error) {
        console.error("Error parsing message:", error, "Raw data:", event.data);
      }
    },

    handleWebSocketError(error) {
      console.error("WebSocket error:", error);
      console.error("  Error details:", {
        type: error.type,
        target: error.target,
        currentTarget: error.currentTarget
      });
    },

    handleWebSocketClose() {
      console.log("WebSocket connection closed");
      console.log("  connectionStatus before:", this.connectionStatus);

      if (this.connectionStatus !== 'disconnected') {
        this.connectionStatus = 'reconnecting';
        console.log("  connectionStatus after:", this.connectionStatus);
        this.scheduleReconnect();
      }
    },

    scheduleReconnect() {
      // Clear any existing reconnect timer
      this.cancelReconnect();

      if (this.reconnectAttempts >= this.maxReconnectAttempts) {
        console.error("Max reconnection attempts reached");
        this.connectionStatus = 'disconnected';
        return;
      }

      // Calculate delay with exponential backoff
      const delay = Math.min(
        this.reconnectDelay * Math.pow(2, this.reconnectAttempts),
        30000 // Max 30 seconds
      );

      console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts + 1}/${this.maxReconnectAttempts})`);

      this.reconnectTimer = setTimeout(() => {
        this.reconnectAttempts++;
        this.connectWebSocket();
      }, delay);
    },

    cancelReconnect() {
      if (this.reconnectTimer) {
        clearTimeout(this.reconnectTimer);
        this.reconnectTimer = null;
      }
    },

    // Game board update
    updateBoard() {
      // Only send updates if we are master
      if (this.socket && this.socket.readyState === WebSocket.OPEN && this.clientRole === 'master') {
        this.socket.send(JSON.stringify({
          type: 'game_state_update',
          boardData: this.boardData
        }));
      }
    },
    initParams: function () {
      this.first_base = false;
      this.second_base = false;
      this.third_base = false;
      this.ball_cnt = 0;
      this.strike_cnt = 0;
      this.out_cnt = 0;
    },
    resetBSO: function () {
      this.ball_cnt = 0;
      this.strike_cnt = 0;
      this.out_cnt = 0;
    },
    resetBS: function () {
      this.ball_cnt = 0;
      this.strike_cnt = 0;
    },
    inningMsg: function () {
      if (this.game_inning < 1) {
        return "試合前";
      } else if (this.game_inning > this.last_inning) {
        return "試合終了";
      } else {
        return this.game_inning + "回" + (this.top ? "オモテ" : "ウラ");
      }
    },
    changeOffense: function () {
      if (this.top) {
        this.top = false;
      } else {
        this.top = true;
      }
      this.initParams();
    },
    isPlaying: function () {
      if (this.game_inning >= 1 && this.game_inning <= this.last_inning) {
        return true;
      } else {
        return false;
      }
    },
    ballCountUp: function () {
      if (this.ball_cnt < 3) {
        this.ball_cnt++;
      }
    },
    ballCountDown: function () {
      if (this.ball_cnt >= 1) {
        this.ball_cnt--;
      }
    },
    strikeCountUp: function () {
      if (this.strike_cnt < 2) {
        this.strike_cnt++;
      }
    },
    strikeCountDown: function () {
      if (this.strike_cnt >= 1) {
        this.strike_cnt--;
      }
    },
    outCountUp: function () {
      if (this.out_cnt < 2) {
        this.out_cnt++;
      }
    },
    outCountDown: function () {
      if (this.out_cnt >= 1) {
        this.out_cnt--;
      }
    },
    gameStatusUp: function () {
      if (this.game_inning < this.last_inning + 1) {
        this.game_inning++;
        this.initParams();
      }
    },
    gameStatusDown: function () {
      if (this.game_inning > 0) {
        this.game_inning--;
        this.initParams();
      }
    },
    gameForward: function () {
      if (this.game_inning <= this.last_inning) {
        if (this.top) {
          this.top = false;
        } else {
          this.game_inning++;
          this.top = true;
        }
        this.initParams();
      }
    },
    gameBackward: function () {
      if (this.game_inning >= 1) {
        if (this.top) {
          this.game_inning--;
          this.top = false;
        } else {
          this.top = true;
        }
        this.initParams();
        if (this.game_inning === 0) {
          this.score_top = 0;
          this.score_bottom = 0;
        }
      }
    },
    incrementScoreTop: function () {
      this.score_top++;
    },
    decrementScoreTop: function () {
      if (this.score_top > 0) {
        this.score_top--;
      }
    },
    incrementScoreBottom: function () {
      this.score_bottom++;
    },
    decrementScoreBottom: function () {
      if (this.score_bottom > 0) {
        this.score_bottom--;
      }
    },
    resetGame: function () {
      // Confirmation dialog with extra warning if game is in progress
      let confirmMessage = '試合を初期化してよろしいですか？\n\nイニング、得点、BSO、ランナーがすべてリセットされます。';

      if (this.game_inning >= 1 && this.game_inning <= this.last_inning) {
        confirmMessage = '⚠️ 試合中ですが、本当に初期化しますか？\n\nイニング、得点、BSO、ランナーがすべてリセットされます。';
      }

      if (!confirm(confirmMessage)) {
        return;
      }

      // Reset game state to initial values
      this.game_inning = 0;      // Before game starts
      this.top = true;            // Top of inning (offensive team)
      this.score_top = 0;         // Reset top team score
      this.score_bottom = 0;      // Reset bottom team score
      this.initParams();          // Reset BSO and runners
    },
    endGame: function () {
      // Set game_inning to last_inning + 1 to display "試合終了"
      this.game_inning = this.last_inning + 1;
      this.initParams();
    },
    releaseMasterControl: function () {
      if (this.clientRole !== 'master') return;

      if (!confirm('マスター権限を解放してよろしいですか？\n\n他の接続中のクライアントがマスターになります。')) {
        return;
      }

      if (this.socket && this.socket.readyState === WebSocket.OPEN) {
        this.socket.send(JSON.stringify({
          type: 'release_master'
        }));

        // Clear token from sessionStorage (server will also send clearToken in role_changed)
        sessionStorage.removeItem('masterToken');
        console.log('Master token removed from sessionStorage (manual release)');
      }
    },
    loadFromInitData: function () {
      // Confirmation dialog
      let confirmMessage = '新規大会で初期化してよろしいですか？\n\n';
      confirmMessage += 'config/init_data.json から大会設定を読み込み、\n';
      confirmMessage += '試合状況（イニング、得点、BSO、ランナー）をすべてリセットします。';

      if (this.game_inning >= 1 && this.game_inning <= this.last_inning) {
        confirmMessage = '⚠️ 試合中ですが、本当に新規大会で初期化しますか？\n\n' + confirmMessage;
      }

      if (!confirm(confirmMessage)) {
        return;
      }

      // Load init_data.json
      fetch("/init_data.json")
        .then((response) => {
          if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
          }
          return response.json();
        })
        .then((data) => {
          // Load all fields from init_data.json
          this.game_title = data.game_title || '';
          this.team_top = data.team_top || '';
          this.team_bottom = data.team_bottom || '';
          this.last_inning = data.last_inning || 9;

          // Update UI configuration (dropdown options)
          this.game_array = data.game_array || [];
          this.team_items = data.team_items || [];

          // Reset game state to initial values
          this.game_inning = 0;      // Before game starts
          this.top = true;            // Top of inning
          this.score_top = 0;         // Reset top team score
          this.score_bottom = 0;      // Reset bottom team score
          this.initParams();          // Reset BSO and runners

          console.log('新規大会で初期化しました:', data.game_title);
        })
        .catch((error) => {
          console.error('init_data.json の読み込みに失敗しました:', error);
          alert('エラー: 大会設定ファイル (init_data.json) の読み込みに失敗しました。\n\n' + error.message);
        });
    },
  },
});
app.component("scoreboard", scoreboardComponent);
app.mount("#app");
