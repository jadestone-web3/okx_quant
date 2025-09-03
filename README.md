# 海龟量化交易系统

一个基于Rust开发的量化交易系统，实现了经典的海龟交易策略，支持实时数据获取、策略回测和自动交易。

## 功能特性

- 🚀 **实时数据获取**: 通过OKX WebSocket接口获取实时tick数据
- 📊 **历史数据管理**: REST API获取K线数据并存储到SQLite数据库
- 🧠 **海龟策略**: 实现经典的海龟交易法则
- 📈 **回测分析**: 基于历史数据的策略回测和性能分析
- 💼 **风险管理**: 基于ATR的仓位管理和风险控制
- 📱 **实时交易**: 根据策略信号执行自动交易
- 📋 **详细报告**: 生成包含收益率、最大回撤、夏普比率等指标的回测报告

## 技术架构

### 项目结构
```
quant_trader/
├── src/
│   ├── main.rs              # 主程序入口
│   ├── types.rs             # 数据类型定义
│   ├── db.rs                # 数据库操作模块
│   ├── handler.rs           # 数据处理模块
│   ├── strategy.rs          # 海龟策略实现
│   └── strategy_manager.rs  # 策略管理模块
├── Cargo.toml              # 依赖配置
└── README.md               # 使用说明
```

### 核心模块

- **数据获取模块 (handler.rs)**: 负责从OKX获取实时和历史数据
- **数据存储模块 (db.rs)**: SQLite数据库的CRUD操作
- **策略模块 (strategy.rs)**: 海龟交易策略的核心实现
- **策略管理模块 (strategy_manager.rs)**: 策略执行、回测、风险管理
- **类型定义 (types.rs)**: 所有数据结构的定义

## 快速开始

### 环境要求
- Rust 1.70+
- SQLite 3

### 安装运行

1. **克隆项目**
```bash
git clone <project-url>
cd quant_trader
```

2. **编译项目**
```bash
cargo build --release
```

3. **运行程序**
```bash
cargo run
```

### 使用说明

程序启动后会显示菜单选项：

```
请选择功能:
1. 开始数据收集
2. 运行回测
3. 实时交易
4. 查看交易历史
5. 退出
```

- **选项1**: 开始从OKX收集SOL-USDT的实时数据并存储到数据库
- **选项2**: 基于历史数据运行海龟策略回测
- **选项3**: 启动实时交易监控（实际交易需要API密钥）
- **选项4**: 查看最近的交易记录

## 海龟策略说明

### 策略原理

海龟交易法则是一个完整的趋势跟踪系统，包含以下核心要素：

1. **入场规则**: 
   - 多头入场：价格突破过去20日最高价
   - 空头入场：价格跌破过去20日最低价

2. **离场规则**:
   - 多头离场：价格跌破过去10日最低价
   - 空头离场：价格突破过去10日最高价

3. **仓位管理**:
   - 基于ATR计算仓位大小
   - 每笔交易风险控制在账户资金的2%以内

### 策略参数

```rust
pub struct TurtleParams {
    pub entry_period: usize,     // 入场周期 (默认20)
    pub exit_period: usize,      // 离场周期 (默认10)
    pub atr_period: usize,       // ATR周期 (默认20)
    pub risk_per_trade: f64,     // 每笔交易风险 (默认0.02)
    pub max_units: usize,        // 最大仓位单位 (默认4)
}
```

## 数据库设计

### 主要数据表

1. **candles表**: K线数据
   - timestamp: 时间戳
   - symbol: 交易对
   - open/high/low/close: OHLC价格
   - volume: 成交量

2. **tickers表**: 实时tick数据
   - timestamp: 时间戳
   - symbol: 交易对
   - last_price/bid_price/ask_price: 价格信息

3. **signals表**: 交易信号
   - timestamp: 信号时间
   - signal_type: 信号类型(买/卖/持有)
   - price: 触发价格
   - confidence: 置信度

4. **trades表**: 交易记录
   - timestamp: 交易时间
   - side: 买卖方向
   - price: 成交价格
   - quantity: 数量
   - pnl: 盈亏

## 回测报告示例

```
===== 回测报告 =====
初始资金: $10000.00
最终资金: $12450.30
总收益: $2450.30
收益率: 24.50%
最大回撤: 8.32%
交易次数: 45
胜率: 62.22%
平均收益: $54.45
夏普比率: 1.83
```

## 风险管理

### 仓位计算公式
```
风险资金 = 账户总资金 × 风险比例(2%)
单位风险 = ATR × 价格系数
仓位大小 = 风险资金 / 单位风险
```

### 风险控制措施
- 单笔交易最大风险限制在账户资金的2%
- 基于ATR动态调整仓位大小
- 支持最大仓位单位限制
- 实时监控未实现盈亏

## API接口说明

### OKX WebSocket接口
- **地址**: `wss://ws.okx.com:8443/ws/v5/public`
- **订阅频道**: `tickers`
- **交易对**: `SOL-USDT`

### OKX REST API
- **K线接口**: `https://www.okx.com/api/v5/market/candles`
- **参数**: `instId=SOL-USDT&bar=1m&limit=1000`

## 扩展开发

### 添加新策略

1. 在`strategy.rs`中实现新的策略结构体
2. 实现`analyze()`方法生成交易信号
3. 在`strategy_manager.rs`中添加策略管理逻辑

### 添加新交易对

1. 修改`handler.rs`中的WebSocket订阅
2. 在`strategy_manager.rs`中添加新的策略实例
3. 更新数据库查询以支持多交易对

### 自定义指标

在`strategy.rs`中添加技术指标计算函数：
```rust
fn calculate_custom_indicator(&self, candles: &[CandleData]) -> Result<f64> {
    // 实现自定义指标逻辑
}
```

## 注意事项

⚠️ **重要提醒**:
1. 本系统仅供学习和研究使用
2. 实际交易存在风险，请谨慎使用
3. 需要OKX API密钥才能执行真实交易
4. 建议先在测试环境中验证策略效果
5. 策略参数需要根据市场条件进行优化

## 性能优化

- 使用异步编程提高并发性能
- SQLite索引优化查询性能
- 批量数据插入减少I/O开销
- 内存缓存热点数据
- 连接池管理数据库连接

## 监控和日志

系统使用`log`和`env_logger`进行日志记录：

```bash
# 设置日志级别
export RUST_LOG=info
cargo run

# 详细调试信息
export RUST_LOG=debug
cargo run
```

日志级别说明：
- `error`: 系统错误
- `warn`: 警告信息
- `info`: 一般信息
- `debug`: 调试信息

## 配置管理

### 环境变量配置

```bash
# 数据库路径
export DB_PATH="./trading.db"

# API配置
export OKX_API_KEY="your-api-key"
export OKX_SECRET_KEY="your-secret-key"
export OKX_PASSPHRASE="your-passphrase"

# 策略参数
export TURTLE_ENTRY_PERIOD=20
export TURTLE_EXIT_PERIOD=10
export TURTLE_ATR_PERIOD=20
export TURTLE_RISK_PER_TRADE=0.02
```

## 故障排除

### 常见问题

1. **WebSocket连接失败**
   - 检查网络连接
   - 确认OKX服务状态
   - 查看防火墙设置

2. **数据库锁定错误**
   - 确保只有一个程序实例在运行
   - 检查数据库文件权限
   - 重启程序释放锁定

3. **API请求限制**
   - OKX有请求频率限制
   - 增加请求间隔时间
   - 使用WebSocket获取实时数据

4. **内存使用过高**
   - 定期清理历史数据
   - 优化数据结构
   - 增加数据分页处理

### 调试技巧

1. **启用详细日志**
```bash
RUST_LOG=debug cargo run
```

2. **检查数据库内容**
```bash
sqlite3 trading.db
.tables
SELECT COUNT(*) FROM candles;
```

3. **监控系统资源**
```bash
htop
iostat -x 1
```

## 测试

### 单元测试
```bash
cargo test
```

### 集成测试
```bash
cargo test --test integration_tests
```

### 回测验证
```bash
# 运行历史数据回测
cargo run
# 选择选项2进行回测
```

## 部署建议

### 生产环境部署

1. **编译优化版本**
```bash
cargo build --release
```

2. **配置系统服务**
```bash
# 创建systemd服务文件
sudo tee /etc/systemd/system/quant-trader.service << EOF
[Unit]
Description=Quant Trading System
After=network.target

[Service]
Type=simple
User=trader
WorkingDirectory=/opt/quant-trader
ExecStart=/opt/quant-trader/target/release/quant_trader
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl enable quant-trader
sudo systemctl start quant-trader
```

3. **日志轮转**
```bash
# 配置logrotate
sudo tee /etc/logrotate.d/quant-trader << EOF
/var/log/quant-trader/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    copytruncate
}
EOF
```

### 容器化部署

```dockerfile
# Dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    sqlite3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/quant_trader .
COPY --from=builder /app/Cargo.toml .

EXPOSE 8080
CMD ["./quant_trader"]
```

```bash
# 构建和运行
docker build -t quant-trader .
docker run -d --name quant-trader \
  -v /data/trading.db:/app/trading.db \
  -e RUST_LOG=info \
  quant-trader
```

## 贡献指南

### 代码规范

1. **命名规范**
   - 结构体使用PascalCase
   - 函数和变量使用snake_case
   - 常量使用SCREAMING_SNAKE_CASE

2. **文档注释**
   - 所有公共函数必须有文档注释
   - 使用中文注释说明业务逻辑
   - 示例代码使用英文注释

3. **错误处理**
   - 使用`anyhow`进行错误处理
   - 关键错误必须记录日志
   - 优雅处理网络和数据库错误

### 提交规范

```bash
# 功能开发
git commit -m "feat: 添加MACD策略支持"

# 错误修复
git commit -m "fix: 修复WebSocket重连问题"

# 文档更新
git commit -m "docs: 更新API使用说明"

# 代码重构
git commit -m "refactor: 优化数据库查询性能"
```

## 许可证

本项目采用MIT许可证，详见LICENSE文件。

## 联系方式

- 项目主页: [GitHub Repository]
- 问题反馈: [GitHub Issues]
- 邮箱: developer@example.com

---

**免责声明**: 本软件仅供学习和研究使用。投资有风险，使用本软件进行实际交易时请谨慎评估风险。开发者不对使用本软件造成的任何损失承担责任。