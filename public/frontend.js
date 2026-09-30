const { useState, useEffect } = React;
// const { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } = Recharts;


// API Service
const API = {
    baseUrl: 'http://localhost:3000/api',
    
    async request(endpoint, options = {}) {
        const token = localStorage.getItem('token');
        const headers = {
            'Content-Type': 'application/json',
            ...(token && { 'Authorization': `Bearer ${token}` }),
            ...options.headers
        };
        
        const response = await fetch(`${this.baseUrl}${endpoint}`, {
            ...options,
            headers
        });
        
        const data = await response.json();
        if (!response.ok) throw new Error(data.error || 'Request failed');
        return data;
    },
    
    // Auth
    async register(email, password, name) {
        return this.request('/auth/register', {
            method: 'POST',
            body: JSON.stringify({ email, password, name })
        });
    },
    
    async login(email, password) {
        return this.request('/auth/login', {
            method: 'POST',
            body: JSON.stringify({ email, password })
        });
    },
    
    // Markets
    async getMarkets() {
        return this.request('/markets');
    },
    
    async getOrderBook(marketId) {
        return this.request(`/markets/${marketId}/orderbook`);
    },
    
    async getTrades(marketId) {
        return this.request(`/markets/${marketId}/trades`);
    },
    
    // Trading
    async placeOrder(order) {
        return this.request('/orders', {
            method: 'POST',
            body: JSON.stringify(order)
        });
    },
    
    async getOrders() {
        return this.request('/orders');
    },
    
    async cancelOrder(orderId) {
        return this.request(`/orders/${orderId}`, {
            method: 'DELETE'
        });
    },
    
    // Account
    async getBalance() {
        return this.request('/account/balance');
    },
    
    async getHistory() {
        return this.request('/account/history');
    }
};

// WebSocket Service
class WebSocketService {
    constructor() {
        this.ws = null;
        this.listeners = new Map();
    }
    
    connect() {
        this.ws = new WebSocket('ws://localhost:3000');
        
        this.ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            const listeners = this.listeners.get(data.type) || [];
            listeners.forEach(callback => callback(data));
        };
        
        this.ws.onclose = () => {
            console.log('WebSocket disconnected');
            setTimeout(() => this.connect(), 5000);
        };
    }
    
    subscribe(marketId) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify({ type: 'subscribe', marketId }));
        }
    }
    
    unsubscribe(marketId) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify({ type: 'unsubscribe', marketId }));
        }
    }
    
    on(eventType, callback) {
        if (!this.listeners.has(eventType)) {
            this.listeners.set(eventType, []);
        }
        this.listeners.get(eventType).push(callback);
    }
    
    removeListener(eventType, callback) {
        const listeners = this.listeners.get(eventType) || [];
        const index = listeners.indexOf(callback);
        if (index !== -1) listeners.splice(index, 1);
    }
}

const wsService = new WebSocketService();

// Login Component
function Login({ onLogin }) {
    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [name, setName] = useState('');
    const [isRegister, setIsRegister] = useState(false);
    const [error, setError] = useState('');
    
    const handleSubmit = async (e) => {
        e.preventDefault();
        setError('');
        
        try {
            if (isRegister) {
                const data = await API.register(email, password, name);
                localStorage.setItem('token', data.token);
                onLogin(data.user);
            } else {
                const data = await API.login(email, password);
                localStorage.setItem('token', data.token);
                onLogin(data.user);
            }
        } catch (err) {
            setError(err.message);
        }
    };
    
    return (
        <div className="login-container">
            <div className="login-box">
                <h2>{isRegister ? 'Create Account' : 'Welcome Back'}</h2>
                {error && <div className="error-message">{error}</div>}
                <form onSubmit={handleSubmit}>
                    {isRegister && (
                        <div className="form-group">
                            <label>Name</label>
                            <input
                                type="text"
                                value={name}
                                onChange={(e) => setName(e.target.value)}
                                placeholder="Your name"
                            />
                        </div>
                    )}
                    <div className="form-group">
                        <label>Email</label>
                        <input
                            type="email"
                            value={email}
                            onChange={(e) => setEmail(e.target.value)}
                            placeholder="test@example.com"
                            required
                        />
                    </div>
                    <div className="form-group">
                        <label>Password</label>
                        <input
                            type="password"
                            value={password}
                            onChange={(e) => setPassword(e.target.value)}
                            placeholder={isRegister ? 'Create password' : 'Your password'}
                            required
                        />
                    </div>
                    <button type="submit" className="btn-primary">
                        {isRegister ? 'Register' : 'Login'}
                    </button>
                </form>
                <p className="toggle-auth">
                    {isRegister ? 'Already have an account?' : "Don't have an account?"}
                    <button onClick={() => setIsRegister(!isRegister)} className="link-btn">
                        {isRegister ? 'Login' : 'Register'}
                    </button>
                </p>
                <p className="test-credentials">
                    Test: test@example.com / password123
                </p>
            </div>
        </div>
    );
}

// Market Card Component
function MarketCard({ market, onSelect }) {
    const changeColor = market.change24h >= 0 ? 'positive' : 'negative';
    
    return (
        <div className="market-card" onClick={() => onSelect(market)}>
            <div className="market-header">
                <h3>{market.name}</h3>
                {market.isPrediction && <span className="prediction-badge">Prediction</span>}
            </div>
            <div className="market-price">${market.currentPrice.toFixed(2)}</div>
            <div className={`market-change ${changeColor}`}>
                {market.change24h >= 0 ? '▲' : '▼'} {Math.abs(market.change24h)}%
            </div>
            <div className="market-volume">Vol: ${(market.volume24h / 1e6).toFixed(1)}M</div>
        </div>
    );
}

// Order Book Component
function OrderBook({ marketId }) {
    const [orderBook, setOrderBook] = useState({ bids: [], asks: [] });
    
    useEffect(() => {
        const fetchOrderBook = async () => {
            try {
                const data = await API.getOrderBook(marketId);
                setOrderBook(data);
            } catch (err) {
                console.error('Failed to fetch order book:', err);
            }
        };
        
        fetchOrderBook();
        const interval = setInterval(fetchOrderBook, 2000);
        
        return () => clearInterval(interval);
    }, [marketId]);
    
    return (
        <div className="order-book">
            <h3>Order Book</h3>
            <div className="order-book-header">
                <span>Price</span>
                <span>Size</span>
                <span>Total</span>
            </div>
            <div className="asks">
                {orderBook.asks.slice().reverse().map((ask, i) => (
                    <div key={i} className="order-row ask">
                        <span className="price">${ask.price.toFixed(2)}</span>
                        <span>{ask.size.toFixed(4)}</span>
                        <span>${(ask.price * ask.size).toFixed(2)}</span>
                    </div>
                ))}
            </div>
            <div className="spread">
                Spread: ${(orderBook.asks[0]?.price - orderBook.bids[0]?.price).toFixed(2)}
            </div>
            <div className="bids">
                {orderBook.bids.map((bid, i) => (
                    <div key={i} className="order-row bid">
                        <span className="price">${bid.price.toFixed(2)}</span>
                        <span>{bid.size.toFixed(4)}</span>
                        <span>${(bid.price * bid.size).toFixed(2)}</span>
                    </div>
                ))}
            </div>
        </div>
    );
}

// Trading Form Component
function TradingForm({ market, onOrderPlaced }) {
    const [side, setSide] = useState('buy');
    const [type, setType] = useState('limit');
    const [price, setPrice] = useState(market?.currentPrice || 0);
    const [quantity, setQuantity] = useState('');
    const [error, setError] = useState('');
    
    const handleSubmit = async (e) => {
        e.preventDefault();
        setError('');
        
        try {
            await API.placeOrder({
                marketId: market.id,
                side,
                type,
                price: type === 'limit' ? parseFloat(price) : undefined,
                quantity: parseFloat(quantity)
            });
            
            setQuantity('');
            onOrderPlaced();
        } catch (err) {
            setError(err.message);
        }
    };
    
    return (
        <div className="trading-form">
            <h3>Place Order</h3>
            <div className="order-type-selector">
                <button
                    className={side === 'buy' ? 'active' : ''}
                    onClick={() => setSide('buy')}
                >
                    Buy
                </button>
                <button
                    className={side === 'sell' ? 'active' : ''}
                    onClick={() => setSide('sell')}
                >
                    Sell
                </button>
            </div>
            <div className="order-type-selector">
                <button
                    className={type === 'limit' ? 'active' : ''}
                    onClick={() => setType('limit')}
                >
                    Limit
                </button>
                <button
                    className={type === 'market' ? 'active' : ''}
                    onClick={() => setType('market')}
                >
                    Market
                </button>
            </div>
            {error && <div className="error-message">{error}</div>}
            <form onSubmit={handleSubmit}>
                {type === 'limit' && (
                    <div className="form-group">
                        <label>Price ($)</label>
                        <input
                            type="number"
                            value={price}
                            onChange={(e) => setPrice(e.target.value)}
                            step="0.01"
                            min="0"
                            required
                        />
                    </div>
                )}
                <div className="form-group">
                    <label>Quantity</label>
                    <input
                        type="number"
                        value={quantity}
                        onChange={(e) => setQuantity(e.target.value)}
                        step="0.0001"
                        min="0.0001"
                        required
                    />
                </div>
                <div className="form-group">
                    <label>Total</label>
                    <input
                        type="text"
                        value={`$${(parseFloat(price || 0) * parseFloat(quantity || 0)).toFixed(2)}`}
                        disabled
                    />
                </div>
                <button type="submit" className="btn-primary">
                    Place {side === 'buy' ? 'Buy' : 'Sell'} Order
                </button>
            </form>
        </div>
    );
}

// Price Chart Component
// function PriceChart({ marketId }) {
//     const [data, setData] = useState([]);
    
//     useEffect(() => {
//         // Generate sample price data
//         const generateData = () => {
//             const points = [];
//             let price = 50000;
//             for (let i = 30; i >= 0; i--) {
//                 price = price * (1 + (Math.random() - 0.5) * 0.02);
//                 points.push({
//                     time: i,
//                     price: price
//                 });
//             }
//             setData(points);
//         };
        
//         generateData();
//         const interval = setInterval(generateData, 10000);
        
//         return () => clearInterval(interval);
//     }, [marketId]);
    
//     return (
//         <div className="price-chart">
//             <h3>Price Chart</h3>
//             <ResponsiveContainer width="100%" height={300}>
//                 <LineChart data={data}>
//                     <CartesianGrid strokeDasharray="3 3" />
//                     <XAxis dataKey="time" />
//                     <YAxis domain={['auto', 'auto']} />
//                     <Tooltip />
//                     <Line type="monotone" dataKey="price" stroke="#8884d8" />
//                 </LineChart>
//             </ResponsiveContainer>
//         </div>
//     );
// }

// Main App Component
function App() {
    const [user, setUser] = useState(null);
    const [markets, setMarkets] = useState([]);
    const [selectedMarket, setSelectedMarket] = useState(null);
    const [balance, setBalance] = useState(0);
    const [orders, setOrders] = useState([]);
    const [activeTab, setActiveTab] = useState('markets');
    
    useEffect(() => {
        wsService.connect();
        
        wsService.on('price_update', (data) => {
            setMarkets(prev => prev.map(m =>
                m.id === data.marketId
                    ? { ...m, currentPrice: data.price }
                    : m
            ));
        });
        
        return () => {
            // Cleanup
        };
    }, []);
    
    useEffect(() => {
        if (user) {
            fetchMarkets();
            fetchBalance();
            fetchOrders();
        }
    }, [user]);
    
    const fetchMarkets = async () => {
        try {
            const data = await API.getMarkets();
            setMarkets(data);
        } catch (err) {
            console.error('Failed to fetch markets:', err);
        }
    };
    
    const fetchBalance = async () => {
        try {
            const data = await API.getBalance();
            setBalance(data.balance);
        } catch (err) {
            console.error('Failed to fetch balance:', err);
        }
    };
    
    const fetchOrders = async () => {
        try {
            const data = await API.getOrders();
            setOrders(data);
        } catch (err) {
            console.error('Failed to fetch orders:', err);
        }
    };
    
    const handleLogout = () => {
        localStorage.removeItem('token');
        setUser(null);
    };
    
    if (!user) {
        return <Login onLogin={setUser} />;
    }
    
    return (
        <div className="app">
            <header className="header">
                <h1>Trading Prediction Platform</h1>
                <div className="user-info">
                    <span>Welcome, {user.name}</span>
                    <span className="balance">Balance: ${balance.toFixed(2)}</span>
                    <button onClick={handleLogout} className="logout-btn">Logout</button>
                </div>
            </header>
            
            <nav className="nav-tabs">
                <button
                    className={activeTab === 'markets' ? 'active' : ''}
                    onClick={() => setActiveTab('markets')}
                >
                    Markets
                </button>
                <button
                    className={activeTab === 'orders' ? 'active' : ''}
                    onClick={() => setActiveTab('orders')}
                >
                    My Orders
                </button>
                <button
                    className={activeTab === 'account' ? 'active' : ''}
                    onClick={() => setActiveTab('account')}
                >
                    Account
                </button>
            </nav>
            
            <main className="main-content">
                {activeTab === 'markets' && (
                    <div className="markets-view">
                        <div className="markets-grid">
                            {markets.map(market => (
                                <MarketCard
                                    key={market.id}
                                    market={market}
                                    onSelect={() => {
                                        setSelectedMarket(market);
                                        wsService.subscribe(market.id);
                                    }}
                                />
                            ))}
                        </div>
                        
                        {selectedMarket && (
                            <div className="market-detail">
                                <div className="market-detail-header">
                                    <h2>{selectedMarket.name}</h2>
                                    <button
                                        className="close-btn"
                                        onClick={() => {
                                            wsService.unsubscribe(selectedMarket.id);
                                            setSelectedMarket(null);
                                        }}
                                    >
                                        ×
                                    </button>
                                </div>
                                
                                <div className="market-detail-content">
                                    <div className="left-panel">
                                        <PriceChart marketId={selectedMarket.id} />
                                        <OrderBook marketId={selectedMarket.id} />
                                    </div>
                                    <div className="right-panel">
                                        <TradingForm
                                            market={selectedMarket}
                                            onOrderPlaced={() => {
                                                fetchOrders();
                                                fetchBalance();
                                            }}
                                        />
                                    </div>
                                </div>
                            </div>
                        )}
                    </div>
                )}
                
                {activeTab === 'orders' && (
                    <div className="orders-view">
                        <h2>My Orders</h2>
                        <table className="orders-table">
                            <thead>
                                <tr>
                                    <th>Market</th>
                                    <th>Side</th>
                                    <th>Type</th>
                                    <th>Price</th>
                                    <th>Quantity</th>
                                    <th>Filled</th>
                                    <th>Status</th>
                                    <th>Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                {orders.map(order => {
                                    const market = markets.find(m => m.id === order.marketId);
                                    return (
                                        <tr key={order.id}>
                                            <td>{market?.symbol || order.marketId}</td>
                                            <td className={order.side}>{order.side}</td>
                                            <td>{order.type}</td>
                                            <td>${order.price?.toFixed(2)}</td>
                                            <td>{order.quantity}</td>
                                            <td>{order.filledQuantity}</td>
                                            <td>
                                                <span className={`status-${order.status}`}>
                                                    {order.status}
                                                </span>
                                            </td>
                                            <td>
                                                {order.status === 'open' && (
                                                    <button
                                                        onClick={async () => {
                                                            await API.cancelOrder(order.id);
                                                            fetchOrders();
                                                        }}
                                                        className="cancel-btn"
                                                    >
                                                        Cancel
                                                    </button>
                                                )}
                                            </td>
                                        </tr>
                                    );
                                })}
                            </tbody>
                        </table>
                    </div>
                )}
                
                {activeTab === 'account' && (
                    <div className="account-view">
                        <h2>Account Overview</h2>
                        <div className="account-info">
                            <div className="info-card">
                                <h3>Profile</h3>
                                <p><strong>Name:</strong> {user.name}</p>
                                <p><strong>Email:</strong> {user.email}</p>
                                <p><strong>Member since:</strong> {new Date(user.createdAt).toLocaleDateString()}</p>
                            </div>
                            
                            <div className="info-card">
                                <h3>Balance</h3>
                                <p className="large-balance">${balance.toFixed(2)}</p>
                                <p><strong>Available:</strong> ${balance.toFixed(2)}</p>
                            </div>
                            
                            <div className="info-card">
                                <h3>Statistics</h3>
                                <p><strong>Total Orders:</strong> {orders.length}</p>
                                <p><strong>Open Orders:</strong> {orders.filter(o => o.status === 'open').length}</p>
                                <p><strong>Filled Orders:</strong> {orders.filter(o => o.status === 'filled').length}</p>
                            </div>
                        </div>
                    </div>
                )}
            </main>
        </div>
    );
}

// Render the app
const root = ReactDOM.createRoot(document.getElementById('root'));
root.render(<App />);