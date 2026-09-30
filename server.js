const express = require('express');
const cors = require('cors');
const http = require('http');
const WebSocket = require('ws');
const bcrypt = require('bcryptjs');
const jwt = require('jsonwebtoken');
const { validateBusinessInfo } = require('./middlewares/validator');
const path = require('path');
const bodyParser = require('body-parser');
const cookieParser = require('cookie-parser');
const { v4: uuidv4 } = require('uuid');
const rustClient = require('./utils/rustClient');

const app = express();
const server = http.createServer(app);
const wss = new WebSocket.Server({ server });

// Middleware
app.use(cors());
app.use(express.json());
app.use(cookieParser());
app.use(bodyParser.urlencoded({ extended: true }));
app.use(express.static('.')); // Serve static files

// Constants
const JWT_SECRET = 'your-super-secret-jwt-key-for-testing-only';
const PORT = 3000;

// In-memory database (for testing)
const users = [];
const markets = [
  {
    id: '1',
    name: 'BTC/USD',
    symbol: 'BTC-USD',
    currentPrice: 52345.67,
    change24h: 2.34,
    volume24h: 1250000000,
    high24h: 53456.78,
    low24h: 51234.56
  },
  {
    id: '2',
    name: 'ETH/USD',
    symbol: 'ETH-USD',
    currentPrice: 3123.45,
    change24h: -1.23,
    volume24h: 850000000,
    high24h: 3245.67,
    low24h: 3098.76
  },
  {
    id: '3',
    name: 'Prediction: BTC > $60k by Dec',
    symbol: 'BTC-DEC-PRED',
    currentPrice: 0.65,
    change24h: 5.67,
    volume24h: 5000000,
    high24h: 0.68,
    low24h: 0.61,
    isPrediction: true,
    expiryDate: '2024-12-31'
  }
];

const orders = [];
const trades = [];
const orderBooks = {
  '1': generateOrderBook(52345.67),
  '2': generateOrderBook(3123.45),
  '3': generateOrderBook(0.65)
};

const user = require('./routes/userRoute');
const product = require('./routes/productRoute');
const order = require('./routes/orderRoute');
const payment = require('./routes/paymentRoute');

app.use('/api/user', user);
app.use('/api/product', product);
app.use('/api/order', order);
app.use('/api/payment', payment);

// Helper functions
function generateOrderBook(basePrice) {
  const bids = [];
  const asks = [];
  
  // Generate bids (buy orders)
  for (let i = 0; i < 10; i++) {
    const price = basePrice * (1 - (i + 1) * 0.001);
    bids.push({
      price: Number(price.toFixed(2)),
      size: Number((Math.random() * 5 + 0.1).toFixed(4))
    });
  }
  
  // Generate asks (sell orders)
  for (let i = 0; i < 10; i++) {
    const price = basePrice * (1 + (i + 1) * 0.001);
    asks.push({
      price: Number(price.toFixed(2)),
      size: Number((Math.random() * 5 + 0.1).toFixed(4))
    });
  }
  
  return { bids, asks };
}

function generateTrades(marketId, count = 20) {
  const market = markets.find(m => m.id === marketId);
  const newTrades = [];
  const now = Date.now();
  
  for (let i = 0; i < count; i++) {
    const price = market.currentPrice * (1 + (Math.random() - 0.5) * 0.002);
    newTrades.push({
      id: uuidv4(),
      marketId,
      price: Number(price.toFixed(2)),
      size: Number((Math.random() * 2 + 0.01).toFixed(4)),
      timestamp: now - i * 60000, // 1 minute apart
      side: Math.random() > 0.5 ? 'buy' : 'sell'
    });
  }
  
  return newTrades;
}

// Authentication middleware
function authenticateToken(req, res, next) {
  const authHeader = req.headers['authorization'];
  const token = authHeader && authHeader.split(' ')[1];
  
  if (!token) {
    return res.status(401).json({ error: 'Authentication required' });
  }
  
  jwt.verify(token, JWT_SECRET, (err, user) => {
    if (err) {
      return res.status(403).json({ error: 'Invalid or expired token' });
    }
    req.user = user;
    next();
  });
}

// WebSocket connection handling
wss.on('connection', (ws) => {
  console.log('WebSocket client connected');
  
  const subscriptions = new Set();
  
  // Send initial data
  ws.send(JSON.stringify({
    type: 'connected',
    message: 'Connected to WebSocket server'
  }));
  
  // Handle messages from client
  ws.on('message', (message) => {
    try {
      const data = JSON.parse(message);
      
      switch (data.type) {
        case 'subscribe':
          if (data.marketId) {
            subscriptions.add(data.marketId);
            console.log(`Client subscribed to market ${data.marketId}`);
          }
          break;
          
        case 'unsubscribe':
          if (data.marketId) {
            subscriptions.delete(data.marketId);
          }
          break;
      }
    } catch (error) {
      console.error('WebSocket message error:', error);
    }
  });
  
  // Send market updates every 2 seconds
  const interval = setInterval(() => {
    subscriptions.forEach(marketId => {
      const market = markets.find(m => m.id === marketId);
      if (market) {
        // Simulate price movement
        const change = (Math.random() - 0.5) * 0.001 * market.currentPrice;
        market.currentPrice += change;
        
        ws.send(JSON.stringify({
          type: 'price_update',
          marketId,
          price: market.currentPrice,
          timestamp: Date.now()
        }));
      }
    });
  }, 2000);
  
  ws.on('close', () => {
    clearInterval(interval);
    console.log('WebSocket client disconnected');
  });
});

// API Routes

// Health check
app.get('/api/health', (req, res) => {
  res.json({ status: 'ok', timestamp: Date.now() });
});

// Auth routes
app.post('/api/auth/register', async (req, res) => {
  const { email, password, name } = req.body;
  
  if (!email || !password) {
    return res.status(400).json({ error: 'Email and password required' });
  }
  
  if (users.find(u => u.email === email)) {
    return res.status(400).json({ error: 'User already exists' });
  }
  
  const hashedPassword = await bcrypt.hash(password, 10);
  const user = {
    id: uuidv4(),
    email,
    name: name || email.split('@')[0],
    password: hashedPassword,
    balance: 10000, // Starting balance
    createdAt: new Date().toISOString()
  };
  
  users.push(user);
  
  const token = jwt.sign({ id: user.id, email: user.email }, JWT_SECRET);
  
  res.json({
    token,
    user: {
      id: user.id,
      email: user.email,
      name: user.name,
      balance: user.balance
    }
  });
});

app.post('/api/auth/login', async (req, res) => {
  const { email, password } = req.body;
  
  const user = users.find(u => u.email === email);
  if (!user) {
    return res.status(401).json({ error: 'Invalid credentials' });
  }
  
  const validPassword = await bcrypt.compare(password, user.password);
  if (!validPassword) {
    return res.status(401).json({ error: 'Invalid credentials' });
  }
  
  const token = jwt.sign({ id: user.id, email: user.email }, JWT_SECRET);
  
  res.json({
    token,
    user: {
      id: user.id,
      email: user.email,
      name: user.name,
      balance: user.balance
    }
  });
});

// Markets routes
app.get('/api/markets', (req, res) => {
  res.json(markets.map(m => ({
    ...m,
    currentPrice: m.currentPrice + (Math.random() - 0.5) * 0.001 * m.currentPrice
  })));
});

app.get('/api/markets/:id', (req, res) => {
  const market = markets.find(m => m.id === req.params.id);
  if (!market) {
    return res.status(404).json({ error: 'Market not found' });
  }
  res.json(market);
});

app.get('/api/markets/:id/orderbook', async (req, res) => {
  try {
    const rustBook = await rustClient.getOrderBook(req.params.id);
    if (rustBook && (rustBook.bids.length > 0 || rustBook.asks.length > 0)) {
      return res.json({
        bids: rustBook.bids.map(b => ({ price: b.price, size: b.amount })),
        asks: rustBook.asks.map(a => ({ price: a.price, size: a.amount }))
      });
    }
  } catch (e) {}

  const orderBook = orderBooks[req.params.id] || generateOrderBook(100);
  
  // Slightly randomize sizes for realism
  const randomizedBook = {
    bids: orderBook.bids.map(b => ({
      ...b,
      size: b.size * (0.9 + Math.random() * 0.2)
    })),
    asks: orderBook.asks.map(a => ({
      ...a,
      size: a.size * (0.9 + Math.random() * 0.2)
    }))
  };
  
  res.json(randomizedBook);
});

app.get('/api/markets/:id/trades', (req, res) => {
  const trades = generateTrades(req.params.id, 20);
  res.json(trades);
});

// Trading routes (protected)
app.post('/api/orders', authenticateToken, async (req, res) => {
  const { marketId, side, type, price, quantity } = req.body;
  
  if (!marketId || !side || !type || !quantity) {
    return res.status(400).json({ error: 'Missing required fields' });
  }
  
  const market = markets.find(m => m.id === marketId);
  if (!market) {
    return res.status(404).json({ error: 'Market not found' });
  }
  
  const order = {
    id: uuidv4(),
    userId: req.user.id,
    marketId,
    side,
    type,
    price: type === 'market' ? market.currentPrice : price,
    quantity,
    filledQuantity: 0,
    status: 'open',
    createdAt: new Date().toISOString()
  };
  
  orders.push(order);
  
  // Send order to Rust Order Matching Engine
  try {
    const rustResp = await rustClient.placeOrder(order);
    if (rustResp && rustResp.order) {
      order.status = rustResp.order.status.toLowerCase();
      order.filledQuantity = rustResp.order.filled_amount;
      if (rustResp.trades && rustResp.trades.length > 0) {
        rustResp.trades.forEach(t => {
          trades.push({
            id: t.id,
            marketId,
            price: t.price,
            size: t.amount,
            timestamp: t.timestamp,
            buyerId: side === 'buy' ? req.user.id : null,
            sellerId: side === 'sell' ? req.user.id : null
          });
        });
      }
      return res.json(order);
    }
  } catch (err) {
    console.warn('Rust engine integration fallback:', err.message);
  }

  // Simulate immediate fill for market orders as fallback
  if (type === 'market') {
    order.status = 'filled';
    order.filledQuantity = quantity;
    
    const trade = {
      id: uuidv4(),
      marketId,
      price: order.price,
      size: quantity,
      timestamp: Date.now(),
      buyerId: side === 'buy' ? req.user.id : null,
      sellerId: side === 'sell' ? req.user.id : null
    };
    trades.push(trade);
  }
  
  res.json(order);
});

app.get('/api/orders', authenticateToken, (req, res) => {
  const userOrders = orders.filter(o => o.userId === req.user.id);
  res.json(userOrders);
});

app.delete('/api/orders/:id', authenticateToken, (req, res) => {
  const orderIndex = orders.findIndex(o => o.id === req.params.id && o.userId === req.user.id);
  
  if (orderIndex === -1) {
    return res.status(404).json({ error: 'Order not found' });
  }
  
  if (orders[orderIndex].status !== 'open') {
    return res.status(400).json({ error: 'Cannot cancel filled or cancelled order' });
  }
  
  orders[orderIndex].status = 'cancelled';
  res.json({ message: 'Order cancelled successfully' });
});

// Account routes (protected)
app.get('/api/account/balance', authenticateToken, (req, res) => {
  const user = users.find(u => u.id === req.user.id);
  res.json({ balance: user.balance });
});

app.get('/api/account/history', authenticateToken, (req, res) => {
  const userTrades = trades.filter(t => t.buyerId === req.user.id || t.sellerId === req.user.id);
  const userOrders = orders.filter(o => o.userId === req.user.id);
  
  res.json({
    trades: userTrades,
    orders: userOrders
  });
});

// Start server
server.listen(PORT, () => {
  console.log(`Server running on http://localhost:${PORT}`);
  console.log(`WebSocket server running on ws://localhost:${PORT}`);
  
  // Create a test user
  const testUser = {
    id: 'test-user-1',
    email: 'test@example.com',
    name: 'Test User',
    password: bcrypt.hashSync('password123', 10),
    balance: 10000,
    createdAt: new Date().toISOString()
  };
  users.push(testUser);
  console.log('Test user created: test@example.com / password123');
});