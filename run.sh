#!/bin/bash

# 海龟量化交易系统启动脚本
# 作者: 量化交易开发大师
# 版本: 1.0.0

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 打印带颜色的消息
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查系统依赖
check_dependencies() {
    print_info "检查系统依赖..."
    
    # 检查Rust环境
    if ! command -v cargo &> /dev/null; then
        print_error "未找到Rust工具链，请先安装Rust"
        echo "安装命令: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    
    # 检查SQLite
    if ! command -v sqlite3 &> /dev/null; then
        print_warning "未找到sqlite3命令行工具，建议安装以便调试"
        echo "Ubuntu/Debian: sudo apt install sqlite3"
        echo "macOS: brew install sqlite"
    fi
    
    print_success "依赖检查完成"
}

# 设置环境变量
setup_environment() {
    print_info "设置环境变量..."
    
    # 默认环境变量
    export RUST_LOG=${RUST_LOG:-info}
    export DB_PATH=${DB_PATH:-"./trading.db"}
    
    # 创建必要的目录
    mkdir -p logs
    mkdir -p data
    
    print_success "环境设置完成"
}

# 编译项目
build_project() {
    print_info "编译项目..."
    
    case $1 in
        "release")
            cargo build --release
            print_success "Release版本编译完成"
            ;;
        "debug"|*)
            cargo build
            print_success "Debug版本编译完成"
            ;;
    esac
}

# 运行测试
run_tests() {
    print_info "运行测试..."
    
    # 单元测试
    cargo test --lib
    
    # 集成测试 (如果存在)
    if [ -d "tests" ]; then
        cargo test --test "*"
    fi
    
    print_success "测试完成"
}

# 初始化数据库
init_database() {
    print_info "初始化数据库..."
    
    # 如果数据库文件已存在，询问是否重新初始化
    if [ -f "$DB_PATH" ]; then
        read -p "数据库文件已存在，是否重新初始化? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            rm -f "$DB_PATH"
            print_info "已删除现有数据库文件"
        else
            print_info "保留现有数据库"
            return 0
        fi
    fi
    
    print_success "数据库初始化准备完成"
}

# 启动程序
start_program() {
    print_info "启动量化交易系统..."
    
    # 检查是否使用release版本
    local binary_path="./target/debug/quant_trader"
    if [ "$1" == "release" ] && [ -f "./target/release/quant_trader" ]; then
        binary_path="./target/release/quant_trader"
    fi
    
    # 启动程序
    if [ -f "$binary_path" ]; then
        print_success "启动程序: $binary_path"
        echo "======================================"
        $binary_path
    else
        print_error "未找到可执行文件，请先编译项目"
        exit 1
    fi
}

# 显示帮助信息
show_help() {
    echo "海龟量化交易系统启动脚本"
    echo ""
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  build          仅编译项目 (debug版本)"
    echo "  build-release  编译release版本"
    echo "  test           运行测试"
    echo "  init-db        初始化数据库"
    echo "  run            编译并运行 (debug版本)"
    echo "  run-release    编译并运行 (release版本)"
    echo "  clean          清理编译文件"
    echo "  help           显示此帮助信息"
    echo ""
    echo "环境变量:"
    echo "  RUST_LOG       日志级别 (debug|info|warn|error)"
    echo "  DB_PATH        数据库文件路径"
    echo ""
    echo "示例:"
    echo "  $0 run                # 运行debug版本"
    echo "  $0 run-release        # 运行release版本"
    echo "  RUST_LOG=debug $0 run # 启用调试日志"
}

# 清理编译文件
clean_project() {
    print_info "清理编译文件..."
    cargo clean
    print_success "清理完成"
}

# 主函数
main() {
    echo "======================================"
    echo "     海龟量化交易系统启动脚本"
    echo "======================================"
    
    case ${1:-run} in
        "build")
            check_dependencies
            setup_environment
            build_project debug
            ;;
        "build-release")
            check_dependencies
            setup_environment
            build_project release
            ;;
        "test")
            check_dependencies
            setup_environment
            run_tests
            ;;
        "init-db")
            setup_environment
            init_database
            ;;
        "run")
            check_dependencies
            setup_environment
            init_database
            build_project debug
            start_program debug
            ;;
        "run-release")
            check_dependencies
            setup_environment
            init_database
            build_project release
            start_program release
            ;;
        "clean")
            clean_project
            ;;
        "help"|"-h"|"--help")
            show_help
            ;;
        *)
            print_error "未知选项: $1"
            show_help
            exit 1
            ;;
    esac
}

# 捕获Ctrl+C信号
trap 'print_info "程序已停止"; exit 0' SIGINT SIGTERM

# 执行主函数
main "$@"