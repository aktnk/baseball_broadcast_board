/**
 * Validates if a color is a valid hex color code
 * @param {string} color - Color string to validate
 * @returns {{valid: boolean, normalizedColor?: string, error?: string}}
 */
function validateHexColor(color) {
  try {
    // Check if color is a non-empty string
    if (typeof color !== 'string' || color.trim() === '') {
      return { valid: false, error: 'Color must be a non-empty string' };
    }

    // Normalize: trim whitespace and convert to lowercase
    const normalizedColor = color.trim().toLowerCase();

    // SECURITY: Check for hex color code format (#rrggbb or #rgb)
    // Only accept standard hex color formats to prevent CSS injection attacks
    const hexColorPattern = /^#([0-9a-f]{6}|[0-9a-f]{3})$/;

    if (!hexColorPattern.test(normalizedColor)) {
      return {
        valid: false,
        error: 'Color must be in hex format (#rrggbb or #rgb). Example: #ff55ff'
      };
    }

    // Expand 3-digit hex to 6-digit for consistency (#rgb -> #rrggbb)
    let expandedColor = normalizedColor;
    if (normalizedColor.length === 4) {
      // #rgb -> #rrggbb
      const r = normalizedColor[1];
      const g = normalizedColor[2];
      const b = normalizedColor[3];
      expandedColor = `#${r}${r}${g}${g}${b}${b}`;
    }

    return { valid: true, normalizedColor: expandedColor };
  } catch (error) {
    return { valid: false, error: error.message };
  }
}

const board = Vue.createApp({
  data: () => ({
    boardData: {
      game_title: "",
      team_top: "",
      team_bottom: "",
      game_inning: 0,
      last_inning: 5,
      top: true,
      first_base: false,
      second_base: false,
      third_base: false,
      ball_cnt: 0,
      strike_cnt: 0,
      out_cnt: 0,
      score_top: 0,
      score_bottom: 0,
    },
    socket: null,
    // WebSocket reconnection
    connectionStatus: 'connecting',
    reconnectAttempts: 0,
    maxReconnectAttempts: 10,
    reconnectDelay: 1000,
    reconnectTimer: null,
    // Background color
    backgroundColor: '#ff55ff',
    // Default color (fallback for invalid colors)
    defaultBackgroundColor: '#ff55ff'
  }),
  async created() {
    // Set default background color immediately (validated)
    this.setBackgroundColor(this.backgroundColor);

    // Initialize WebSocket connection
    this.connectWebSocket();

    // Load background color from Electron settings (Electron mode only)
    if (window.electronAPI) {
      try {
        const color = await window.electronAPI.getBoardBackgroundColor();
        if (color) {
          // SECURITY: Validate color before applying
          this.setBackgroundColor(color);
        }

        // Listen for background color changes
        window.electronAPI.onBoardBackgroundColorChanged((event, color) => {
          // SECURITY: Validate color before applying
          this.setBackgroundColor(color);
          console.log(`Board background color changed to: ${color}`);
        });
      } catch (error) {
        console.error('Failed to load background color from Electron:', error);
      }
    }

    // Load configuration from init_data.json
    fetch("/init_data.json")
      .then((response) => response.json())
      .then((data) => {
        this.boardData.game_title = data.game_title;
        this.boardData.team_top = data.team_top;
        this.boardData.team_bottom = data.team_bottom;

        // Load background color from init_data.json (for browser access)
        // This overrides the default and works for both Web and Electron versions
        if (data.board_background_color) {
          // SECURITY: Validate color before applying
          this.setBackgroundColor(data.board_background_color);
        }
      });
  },
  beforeUnmount() {
    // Clean up WebSocket and timers
    this.cancelReconnect();
    if (this.socket) {
      this.socket.close();
    }
  },
  methods: {
    /**
     * Sets the background color with validation
     * @param {string} color - Color to set (must be valid hex color)
     */
    setBackgroundColor(color) {
      // SECURITY: Validate color before applying to prevent CSS injection
      const validation = validateHexColor(color);

      if (validation.valid) {
        // Use validated and normalized color
        this.backgroundColor = validation.normalizedColor;
        document.body.style.backgroundColor = validation.normalizedColor;
      } else {
        // Invalid color - log warning and use default
        console.warn(
          `Invalid background color received: "${color}". ` +
          `Error: ${validation.error}. Using default color.`
        );
        this.backgroundColor = this.defaultBackgroundColor;
        document.body.style.backgroundColor = this.defaultBackgroundColor;
      }
    },

    // WebSocket connection management
    connectWebSocket() {
      // Dynamically generate WebSocket URL based on current page location
      const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const wsHost = window.location.host;

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
      console.log('WebSocket connection established for display board.');
      this.connectionStatus = 'connected';
      this.reconnectAttempts = 0;

      // Send handshake to identify as board client
      this.socket.send(JSON.stringify({
        type: 'handshake',
        clientType: 'board'
      }));
    },

    handleWebSocketMessage(event) {
      try {
        console.log('Board received WebSocket message:', event.data);
        const message = JSON.parse(event.data);
        console.log('  Parsed message:', message);
        console.log('  Message type:', message.type);

        // Handle game state update
        if (message.type === 'game_state' || !message.type) {
          const newData = message.boardData || message.data || message;
          console.log('  Updating board with data:', newData);

          // Update individual properties to maintain reactivity
          if (newData) {
            // Use 'in' operator to check if property exists (handles false, 0, empty string correctly)
            if ('game_title' in newData) this.boardData.game_title = newData.game_title;
            if ('team_top' in newData) this.boardData.team_top = newData.team_top;
            if ('team_bottom' in newData) this.boardData.team_bottom = newData.team_bottom;
            if ('game_inning' in newData) this.boardData.game_inning = newData.game_inning;
            if ('last_inning' in newData) this.boardData.last_inning = newData.last_inning;
            if ('top' in newData) this.boardData.top = newData.top;
            if ('first_base' in newData) this.boardData.first_base = newData.first_base;
            if ('second_base' in newData) this.boardData.second_base = newData.second_base;
            if ('third_base' in newData) this.boardData.third_base = newData.third_base;
            if ('ball_cnt' in newData) this.boardData.ball_cnt = newData.ball_cnt;
            if ('strike_cnt' in newData) this.boardData.strike_cnt = newData.strike_cnt;
            if ('out_cnt' in newData) this.boardData.out_cnt = newData.out_cnt;
            if ('score_top' in newData) this.boardData.score_top = newData.score_top;
            if ('score_bottom' in newData) this.boardData.score_bottom = newData.score_bottom;
            console.log('  Board updated. New boardData:', this.boardData);
          }
        } else {
          console.log('  Ignoring message type:', message.type);
        }
      } catch (error) {
        console.error('Error parsing board data:', error);
      }
    },

    handleWebSocketError(error) {
      console.error('WebSocket error:', error);
    },

    handleWebSocketClose() {
      console.log('WebSocket connection closed');

      if (this.connectionStatus !== 'disconnected') {
        this.connectionStatus = 'reconnecting';
        this.scheduleReconnect();
      }
    },

    scheduleReconnect() {
      // Clear any existing reconnect timer
      this.cancelReconnect();

      if (this.reconnectAttempts >= this.maxReconnectAttempts) {
        console.error('Max reconnection attempts reached');
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
  },
});
board.component('scoreboard', scoreboardComponent);
board.mount('#board');
