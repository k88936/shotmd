//
// Created by kvto on 9/15/24.
//

// You may need to build the project (run Qt uic code generator) to get "ui_shotmd.h" resolved

#include <QScreen>
#include <QMouseEvent>
#include <QPainter>
#include <QBuffer>
#include <QClipboard>
#include <QTimer>
#include <QStandardPaths>
#include <QDateTime>
#include <QGuiApplication>
#include <QScreen>
#include <QMessageBox>
#include "shotmd.h"
#include "ui_shotmd.h"


shotmd::shotmd(QWidget* parent) : QMainWindow(parent), ui(new Ui::shotmd) {
    setWindowFlags(Qt::WindowStaysOnTopHint | Qt::FramelessWindowHint);
    QScreen* screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen) {
        screen = QGuiApplication::primaryScreen();
    }
    originalPixmap = screen->grabWindow(0);

    screenshotLabel = new QLabel(this);
    screenshotLabel->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    screenshotLabel->setAlignment(Qt::AlignCenter);
    screenshotLabel->setFixedSize(screen->geometry().size());
    screenshotLabel->setPixmap(originalPixmap);
    screenshotLabel->show();

    ui->setupUi(this);
    updateOverlay();
}

void shotmd::mousePressEvent(QMouseEvent* event) {
    if (event->button() == Qt::RightButton) {
        QGuiApplication::exit();
        return;
    }
    if (event->button() == Qt::LeftButton) {
        selection.origin = selection.current = event->pos();
        selection.active = true;
        updateOverlay();
    }
}

void shotmd::mouseMoveEvent(QMouseEvent* event) {
    if (!selection.active) return;
    selection.current = event->pos();
    updateOverlay();
}

void shotmd::updateOverlay() {
    // Start from original screenshot
    QPixmap composited = originalPixmap.copy();
    QPainter painter(&composited);

    // Dim whole screen with semi-transparent layer
    painter.fillRect(composited.rect(), QColor(0, 0, 0, 120)); // 120 alpha black

    if (selection.active) {
        const QRect sel = selection.rect();

        // Restore the selection area ("hole") by drawing original region back
        painter.drawPixmap(sel.topLeft(), originalPixmap.copy(sel));

        // Draw selection border
        QPen pen(Qt::red, 2);
        painter.setPen(pen);
        painter.drawRect(sel);
    }
    painter.end();
    screenshotLabel->setPixmap(composited);
}

void shotmd::captureRegion(const QRect &r) {
    const QPixmap finalPixmap = originalPixmap.copy(r);

    QByteArray data;
    QBuffer buffer(&data);
    finalPixmap.save(&buffer, "JPEG");
    data = data.toBase64();
    const QString html = QString(R"(<img src="data:image/jpeg;base64,)") + data + QString(R"(" alt="screenshot" />)");
    QClipboard *cb = QGuiApplication::clipboard();
    cb->setText(html, QClipboard::Clipboard);
    cb->setText(html, QClipboard::Selection);

    const QString timestamp = QDateTime::currentDateTime().toString("yyyyMMdd-hhmmss");
    QString downloadsPath = QStandardPaths::writableLocation(QStandardPaths::DownloadLocation);
    if (downloadsPath.isEmpty()) downloadsPath = QStandardPaths::writableLocation(QStandardPaths::HomeLocation);
    const QString filePath = downloadsPath + "/" + QString("shotmd-%1.jpeg").arg(timestamp);
    if (!finalPixmap.save(filePath, "JPEG")) {
        QMessageBox::critical(nullptr, "Save Failed",
                              QString("Could not save screenshot to:\n%1\n\nCheck write permissions.").arg(filePath),
                              QMessageBox::Ok);
    }

    //x11 need the clipboard src alive to finish the paste
    hide();
    QTimer::singleShot(60 * 1000, QGuiApplication::instance(), &QGuiApplication::quit);
}

void shotmd::shot(const QPoint Tl, const QPoint RD) { // legacy wrapper
    captureRegion(QRect(Tl, RD));
}

void shotmd::mouseReleaseEvent(QMouseEvent* event) {
    if (!selection.active) return;
    selection.current = event->pos();
    captureRegion(selection.rect());
    selection.reset();
}

void shotmd::keyPressEvent(QKeyEvent *event) {
    if (event->key() == Qt::Key_F) {
        const QScreen *screen = QGuiApplication::primaryScreen();
        captureRegion(screen->geometry());
        return;
    }
    if (event->key() == Qt::Key_Escape)
    {
        QApplication::quit();
    }
    QWidget::keyPressEvent(event);
}


shotmd::~shotmd()
{
    delete ui;
}
