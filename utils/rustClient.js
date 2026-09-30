const WebSocket = require('ws');

class RustServicesClient {
  constructor() {
    this.matchingEngineWs = null;
    this.marketDataWs = null;
    this.isMatchingConnected = false;
    this.isMarketDataConnected = false;
    this.pendingCallbacks = new Map();
    this.reqIdCounter = 1;

    this.symbolMap = {
      '1': 'BTC-USD',
      '2': 'ETH-USD',
      '3': 'BTC-DEC-PRED',
      'BTC-USD': 'BTC-USD',
      'ETH-USD': 'ETH-USD',
      'BTC-DEC-PRED': 'BTC-DEC-PRED'
    };
  }

  resolveSymbol(idOrSymbol) {
    return this.symbolMap[idOrSymbol] || idOrSymbol || 'BTC-USD';
  }

  init() {
    this.connectMatchingEngine();
    this.connectMarketData();
  }

  connectMatchingEngine() {
    try {
      this.matchingEngineWs = new WebSocket('ws://127.0.0.1:8081');

      this.matchingEngineWs.on('open', () => {
        console.log(' Connected to Rust Order Matching Engine (ws://127.0.0.1:8081)');
        this.isMatchingConnected = true;
        // Subscribe to engine events
        this.matchingEngineWs.send(JSON.stringify({ action: 'subscribe' }));
      });

      this.matchingEngineWs.on('message', (data) => {
        try {
          const msg = JSON.parse(data.toString());
          // Handle response or event
        } catch (e) {
          // ignore parsing error
        }
      });

      this.matchingEngineWs.on('close', () => {
        this.isMatchingConnected = false;
        setTimeout(() => this.connectMatchingEngine(), 3000);
      });

      this.matchingEngineWs.on('error', () => {
        this.isMatchingConnected = false;
      });
    } catch (e) {
      this.isMatchingConnected = false;
    }
  }

  connectMarketData() {
    try {
      this.marketDataWs = new WebSocket('ws://127.0.0.1:8082');

      this.marketDataWs.on('open', () => {
        console.log(' Connected to Rust Market Data Service (ws://127.0.0.1:8082)');
        this.isMarketDataConnected = true;
      });

      this.marketDataWs.on('close', () => {
        this.isMarketDataConnected = false;
        setTimeout(() => this.connectMarketData(), 3000);
      });

      this.marketDataWs.on('error', () => {
        this.isMarketDataConnected = false;
      });
    } catch (e) {
      this.isMarketDataConnected = false;
    }
  }

  async placeOrder(orderData) {
    if (!this.isMatchingConnected || !this.matchingEngineWs) {
      return null; // fallback to in-memory if engine not yet ready
    }

    return new Promise((resolve) => {
      const timeout = setTimeout(() => resolve(null), 1000);

      const symbol = this.resolveSymbol(orderData.marketId);
      const side = (orderData.side || 'BUY').toUpperCase();
      const orderType = (orderData.type || 'LIMIT').toUpperCase();

      const rustOrder = {
        id: orderData.id,
        user_id: orderData.userId || 'user',
        symbol: symbol,
        side: side,
        order_type: orderType,
        price: orderData.price ? Number(orderData.price) : null,
        amount: Number(orderData.quantity),
        filled_amount: 0.0,
        remaining_amount: Number(orderData.quantity),
        status: 'NEW',
        created_at: Date.now()
      };

      const handler = (data) => {
        try {
          const msg = JSON.parse(data.toString());
          if (msg.type === 'order_placed' || msg.type === 'error') {
            this.matchingEngineWs.off('message', handler);
            clearTimeout(timeout);
            resolve(msg);
          }
        } catch (e) {}
      };

      this.matchingEngineWs.on('message', handler);
      this.matchingEngineWs.send(JSON.stringify({
        action: 'place_order',
        order: rustOrder
      }));
    });
  }

  async getOrderBook(marketId, depth = 20) {
    if (!this.isMatchingConnected || !this.matchingEngineWs) {
      return null;
    }

    return new Promise((resolve) => {
      const timeout = setTimeout(() => resolve(null), 1000);
      const symbol = this.resolveSymbol(marketId);

      const handler = (data) => {
        try {
          const msg = JSON.parse(data.toString());
          if (msg.type === 'snapshot' && msg.snapshot && msg.snapshot.symbol === symbol) {
            this.matchingEngineWs.off('message', handler);
            clearTimeout(timeout);
            resolve(msg.snapshot);
          }
        } catch (e) {}
      };

      this.matchingEngineWs.on('message', handler);
      this.matchingEngineWs.send(JSON.stringify({
        action: 'get_snapshot',
        symbol: symbol,
        depth: depth
      }));
    });
  }
}

const rustClient = new RustServicesClient();
rustClient.init();

module.exports = rustClient;
